//! Vector store for chunk storage and search

use std::collections::HashMap;
use uuid::Uuid;

use ruvector_core::{VectorDB, VectorEntry, SearchQuery as CoreSearchQuery, DistanceMetric};
use ruvector_core::types::{DbOptions, HnswConfig};

use crate::config::RagConfig;
use crate::error::{Error, Result};
use crate::types::Chunk;

/// Search result with chunk and similarity
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The retrieved chunk
    pub chunk: Chunk,
    /// Similarity score (0.0-1.0, higher is better)
    pub similarity: f32,
}

/// Vector store wrapper for ruvector-core
pub struct VectorStore {
    /// Underlying vector database
    db: VectorDB,
    /// Embedding dimensions
    dimensions: usize,
    /// Mapping from document IDs to chunk IDs for efficient deletion
    document_chunks: parking_lot::RwLock<HashMap<Uuid, Vec<String>>>,
}

impl VectorStore {
    /// Create a new vector store
    pub fn new(config: &RagConfig) -> Result<Self> {
        // Ensure storage directory exists
        if let Some(parent) = config.vector_db.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = DbOptions {
            dimensions: config.embeddings.dimensions,
            distance_metric: DistanceMetric::Cosine,
            storage_path: config.vector_db.storage_path.to_string_lossy().to_string(),
            hnsw_config: Some(HnswConfig {
                m: config.vector_db.hnsw_m,
                ef_construction: config.vector_db.hnsw_ef_construction,
                ef_search: config.vector_db.hnsw_ef_search,
                max_elements: 10_000_000,
            }),
            quantization: None,
        };

        let db = VectorDB::new(options).map_err(|e| Error::VectorDb(e.to_string()))?;

        Ok(Self {
            db,
            dimensions: config.embeddings.dimensions,
            document_chunks: parking_lot::RwLock::new(HashMap::new()),
        })
    }

    /// Insert a chunk into the vector store
    pub fn insert_chunk(&self, chunk: &Chunk) -> Result<()> {
        if chunk.embedding.is_empty() {
            return Err(Error::VectorDb("Chunk has no embedding".to_string()));
        }

        let chunk_id = chunk.id.to_string();

        let entry = VectorEntry {
            id: Some(chunk_id.clone()),
            vector: chunk.embedding.clone(),
            metadata: Some(chunk.to_vector_metadata()),
        };

        self.db.insert(entry).map_err(|e| Error::VectorDb(e.to_string()))?;

        // Track the document-to-chunk mapping
        let mut doc_chunks = self.document_chunks.write();
        doc_chunks
            .entry(chunk.document_id)
            .or_insert_with(Vec::new)
            .push(chunk_id);

        Ok(())
    }

    /// Search for similar chunks
    pub fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        document_filter: Option<&[Uuid]>,
    ) -> Result<Vec<SearchResult>> {
        // Build metadata filter
        let filter = document_filter.map(|_doc_ids| {
            let filter = HashMap::new();
            // For now, we'll filter in post-processing
            // In production, implement proper metadata filtering in ruvector-core
            filter
        });

        let query = CoreSearchQuery {
            vector: query_embedding.to_vec(),
            k: top_k * 2, // Get more for filtering
            filter,
            ef_search: None,
        };

        let results = self.db.search(query).map_err(|e| Error::VectorDb(e.to_string()))?;

        let mut search_results = Vec::new();

        for result in results {
            // Extract chunk from metadata
            if let Some(ref metadata) = result.metadata {
                let chunk = self.metadata_to_chunk(&result.id, metadata)?;

                // Apply document filter
                if let Some(doc_ids) = document_filter {
                    if !doc_ids.contains(&chunk.document_id) {
                        continue;
                    }
                }

                // Convert distance to similarity (cosine distance -> similarity)
                let similarity = 1.0 - result.score.min(2.0) / 2.0;

                search_results.push(SearchResult { chunk, similarity });
            }
        }

        // Sort by similarity and take top_k
        search_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        search_results.truncate(top_k);

        Ok(search_results)
    }

    /// Delete all chunks for a document
    pub fn delete_by_document(&self, document_id: &Uuid) -> Result<usize> {
        // Get chunk IDs for this document from our tracking map
        let chunk_ids = {
            let doc_chunks = self.document_chunks.read();
            doc_chunks.get(document_id).cloned().unwrap_or_default()
        };

        let mut deleted = 0;

        for chunk_id in &chunk_ids {
            if self.db.delete(chunk_id).map_err(|e| Error::VectorDb(e.to_string()))? {
                deleted += 1;
            }
        }

        // Remove from tracking map
        if deleted > 0 {
            let mut doc_chunks = self.document_chunks.write();
            doc_chunks.remove(document_id);
        }

        Ok(deleted)
    }

    /// Get chunk count
    pub fn len(&self) -> Result<usize> {
        self.db.len().map_err(|e| Error::VectorDb(e.to_string()))
    }

    /// Check if empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Convert metadata back to chunk
    fn metadata_to_chunk(
        &self,
        id: &str,
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Result<Chunk> {
        let chunk_id = metadata
            .get("chunk_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| Uuid::parse_str(id).unwrap_or_else(|_| Uuid::new_v4()));

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

        let line_start = metadata
            .get("line_start")
            .and_then(|v| v.as_u64())
            .map(|l| l as u32);

        let line_end = metadata
            .get("line_end")
            .and_then(|v| v.as_u64())
            .map(|l| l as u32);

        let file_type = metadata
            .get("file_type")
            .map(|v| serde_json::from_value(v.clone()).unwrap_or(crate::types::FileType::Unknown))
            .unwrap_or(crate::types::FileType::Unknown);

        let source = crate::types::ChunkSource {
            filename,
            file_type,
            page_number,
            page_count: None,
            section_title,
            heading_hierarchy: Vec::new(),
            sheet_name: None,
            row_range: None,
            line_start,
            line_end,
            code_context: None,
        };

        Ok(Chunk {
            id: chunk_id,
            document_id,
            content,
            embedding: Vec::new(), // Not stored in metadata
            source,
            char_start,
            char_end,
            chunk_index,
            metadata: HashMap::new(),
        })
    }
}
