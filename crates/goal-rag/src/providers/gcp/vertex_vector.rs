//! Vertex AI Vector Search provider
//!
//! Provides managed HNSW vector similarity search.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::auth::GcpAuth;
use crate::error::{Error, Result};
use crate::providers::vector_store::{VectorSearchResult, VectorStoreProvider};
use crate::types::Chunk;

/// Vertex AI Vector Search provider
pub struct VertexVectorSearch {
    auth: Arc<GcpAuth>,
    location: String,
    index_endpoint: String,
    deployed_index_id: String,
    /// Endpoint for data plane operations (upsert, delete)
    data_endpoint: Option<String>,
}

impl VertexVectorSearch {
    /// Create a new Vertex Vector Search provider
    ///
    /// # Arguments
    /// * `auth` - GCP authentication
    /// * `location` - GCP region (e.g., "us-central1")
    /// * `index_endpoint` - Full resource name of the index endpoint
    /// * `deployed_index_id` - ID of the deployed index
    pub fn new(
        auth: Arc<GcpAuth>,
        location: String,
        index_endpoint: String,
        deployed_index_id: String,
    ) -> Self {
        Self {
            auth,
            location,
            index_endpoint,
            deployed_index_id,
            data_endpoint: None,
        }
    }

    /// Set a custom data endpoint for mutations
    pub fn with_data_endpoint(mut self, endpoint: String) -> Self {
        self.data_endpoint = Some(endpoint);
        self
    }

    /// Get search endpoint URL
    fn search_endpoint(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/{}:findNeighbors",
            self.location, self.index_endpoint
        )
    }

    /// Convert chunk to vector search datapoint
    fn chunk_to_datapoint(chunk: &Chunk) -> DataPoint {
        let mut restricts = Vec::new();

        // Add document_id as a restrict for filtering
        restricts.push(Restrict {
            namespace: "document_id".to_string(),
            allow: vec![chunk.document_id.to_string()],
            deny: vec![],
        });

        // Store chunk metadata in crowding tag (up to 1KB)
        let metadata = serde_json::json!({
            "chunk_id": chunk.id.to_string(),
            "document_id": chunk.document_id.to_string(),
            "filename": chunk.source.filename,
            "content": chunk.content.chars().take(500).collect::<String>(),
            "chunk_index": chunk.chunk_index,
            "char_start": chunk.char_start,
            "char_end": chunk.char_end,
            "page_number": chunk.source.page_number,
            "section_title": chunk.source.section_title,
            "file_type": chunk.source.file_type,
        });

        DataPoint {
            datapoint_id: chunk.id.to_string(),
            feature_vector: chunk.embedding.clone(),
            restricts: Some(restricts),
            crowding_tag: Some(CrowdingTag {
                crowding_attribute: metadata.to_string(),
            }),
        }
    }
}

#[derive(serde::Serialize, Clone)]
struct DataPoint {
    datapoint_id: String,
    feature_vector: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restricts: Option<Vec<Restrict>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crowding_tag: Option<CrowdingTag>,
}

#[derive(serde::Serialize, Clone)]
struct Restrict {
    namespace: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allow: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    deny: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
struct CrowdingTag {
    crowding_attribute: String,
}

#[derive(serde::Serialize)]
struct FindNeighborsRequest {
    deployed_index_id: String,
    queries: Vec<QueryItem>,
}

#[derive(serde::Serialize)]
struct QueryItem {
    datapoint: QueryDatapoint,
    neighbor_count: u32,
}

#[derive(serde::Serialize)]
struct QueryDatapoint {
    datapoint_id: String,
    feature_vector: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restricts: Option<Vec<Restrict>>,
}

#[derive(serde::Deserialize)]
struct FindNeighborsResponse {
    nearest_neighbors: Vec<NearestNeighbors>,
}

#[derive(serde::Deserialize)]
struct NearestNeighbors {
    neighbors: Vec<Neighbor>,
}

#[derive(serde::Deserialize)]
struct Neighbor {
    datapoint: NeighborDatapoint,
    distance: f64,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct NeighborDatapoint {
    datapoint_id: String,
    crowding_tag: Option<NeighborCrowdingTag>,
}

#[derive(serde::Deserialize)]
struct NeighborCrowdingTag {
    crowding_attribute: String,
}

#[derive(serde::Serialize)]
struct UpsertRequest {
    datapoints: Vec<DataPoint>,
}

#[derive(serde::Serialize)]
#[allow(dead_code)]
struct RemoveRequest {
    datapoint_ids: Vec<String>,
}

#[async_trait]
impl VectorStoreProvider for VertexVectorSearch {
    async fn insert_chunk(&self, chunk: &Chunk) -> Result<()> {
        self.insert_chunks(&[chunk.clone()]).await
    }

    async fn insert_chunks(&self, chunks: &[Chunk]) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let client = self.auth.authorized_client().await?;

        // Use data endpoint if available, otherwise use streaming update
        let endpoint = self.data_endpoint.clone().unwrap_or_else(|| {
            format!(
                "https://{}-aiplatform.googleapis.com/v1/{}:upsertDatapoints",
                self.location, self.index_endpoint
            )
        });

        // Convert chunks to datapoints
        let datapoints: Vec<DataPoint> = chunks.iter().map(Self::chunk_to_datapoint).collect();

        // Batch upserts (max 100 per request)
        for batch in datapoints.chunks(100) {
            let request = UpsertRequest {
                datapoints: batch.to_vec(),
            };

            let response = client
                .post(&endpoint)
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::VectorDb(format!("Vertex upsert failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(Error::VectorDb(format!(
                    "Vertex upsert failed ({}): {}",
                    status, body
                )));
            }
        }

        Ok(())
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        document_filter: Option<&[Uuid]>,
    ) -> Result<Vec<VectorSearchResult>> {
        let client = self.auth.authorized_client().await?;

        let mut restricts = None;
        if let Some(doc_ids) = document_filter {
            restricts = Some(vec![Restrict {
                namespace: "document_id".to_string(),
                allow: doc_ids.iter().map(|id| id.to_string()).collect(),
                deny: vec![],
            }]);
        }

        let request = FindNeighborsRequest {
            deployed_index_id: self.deployed_index_id.clone(),
            queries: vec![QueryItem {
                datapoint: QueryDatapoint {
                    datapoint_id: "query".to_string(),
                    feature_vector: query_embedding.to_vec(),
                    restricts,
                },
                neighbor_count: top_k as u32,
            }],
        };

        let response = client
            .post(&self.search_endpoint())
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::VectorDb(format!("Vertex search failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::VectorDb(format!(
                "Vertex search failed ({}): {}",
                status, body
            )));
        }

        let search_response: FindNeighborsResponse = response
            .json()
            .await
            .map_err(|e| Error::VectorDb(format!("Failed to parse Vertex response: {}", e)))?;

        let mut results = Vec::new();

        for neighbors in search_response.nearest_neighbors {
            for neighbor in neighbors.neighbors {
                // Parse metadata from crowding tag
                if let Some(crowding) = neighbor.datapoint.crowding_tag {
                    if let Ok(metadata) =
                        serde_json::from_str::<HashMap<String, serde_json::Value>>(
                            &crowding.crowding_attribute,
                        )
                    {
                        let chunk = self.metadata_to_chunk(&metadata)?;

                        // Convert distance to similarity (cosine: similarity = 1 - distance)
                        let similarity = 1.0 - neighbor.distance as f32;

                        results.push(VectorSearchResult { chunk, similarity });
                    }
                }
            }
        }

        Ok(results)
    }

    async fn delete_by_document(&self, document_id: &Uuid) -> Result<usize> {
        // Note: Vertex AI Vector Search requires knowing the datapoint IDs to delete.
        // In a production system, you'd maintain a mapping of document_id -> datapoint_ids
        // For now, we'll return 0 as we can't easily enumerate all chunks for a document
        tracing::warn!(
            "delete_by_document not fully implemented for Vertex AI Vector Search. Document: {}",
            document_id
        );
        Ok(0)
    }

    async fn len(&self) -> Result<usize> {
        // Vertex AI doesn't have a direct count API
        // Would need to query index stats via a different endpoint
        Ok(0)
    }

    async fn health_check(&self) -> Result<bool> {
        self.auth.get_token().await.map(|_| true)
    }

    fn name(&self) -> &str {
        "vertex-vector-search"
    }
}

impl VertexVectorSearch {
    /// Convert metadata back to Chunk
    fn metadata_to_chunk(&self, metadata: &HashMap<String, serde_json::Value>) -> Result<Chunk> {
        let chunk_id = metadata
            .get("chunk_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let document_id = metadata
            .get("document_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let filename = metadata
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let content = metadata
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let chunk_index = metadata
            .get("chunk_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let char_start = metadata
            .get("char_start")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let char_end = metadata
            .get("char_end")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let page_number = metadata
            .get("page_number")
            .and_then(|v| v.as_u64())
            .map(|p| p as u32);

        let section_title = metadata
            .get("section_title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let file_type = metadata
            .get("file_type")
            .map(|v| serde_json::from_value(v.clone()).unwrap_or(crate::types::FileType::Unknown))
            .unwrap_or(crate::types::FileType::Unknown);

        let source = crate::types::ChunkSource {
            filename,
            internal_filename: None,
            file_type,
            page_number,
            page_count: None,
            section_title,
            heading_hierarchy: Vec::new(),
            sheet_name: None,
            row_range: None,
            line_start: None,
            line_end: None,
            code_context: None,
        };

        Ok(Chunk {
            id: chunk_id,
            document_id,
            content,
            embedding: Vec::new(),
            source,
            char_start,
            char_end,
            chunk_index,
            metadata: HashMap::new(),
        })
    }
}
