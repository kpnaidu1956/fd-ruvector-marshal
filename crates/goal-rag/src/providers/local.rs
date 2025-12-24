//! Local provider implementations using filesystem and ruvector-core
//!
//! These wrap the existing VectorStore and provide filesystem document storage.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::RagConfig;
use crate::error::{Error, Result};
use crate::retrieval::VectorStore;
use crate::types::Chunk;

use super::document_store::{DocumentStoreProvider, StoredDocumentInfo};
use super::vector_store::{VectorSearchResult, VectorStoreProvider};

/// Local vector store wrapping ruvector-core HNSW index
pub struct LocalVectorStore {
    store: Arc<VectorStore>,
}

impl LocalVectorStore {
    /// Create from existing VectorStore
    pub fn new(store: Arc<VectorStore>) -> Self {
        Self { store }
    }

    /// Create from config
    pub fn from_config(config: &RagConfig) -> Result<Self> {
        let store = Arc::new(VectorStore::new(config)?);
        Ok(Self { store })
    }

    /// Get underlying store for direct access
    pub fn inner(&self) -> &Arc<VectorStore> {
        &self.store
    }
}

#[async_trait]
impl VectorStoreProvider for LocalVectorStore {
    async fn insert_chunk(&self, chunk: &Chunk) -> Result<()> {
        // VectorStore::insert_chunk is sync, wrap in blocking task
        let store = self.store.clone();
        let chunk = chunk.clone();
        tokio::task::spawn_blocking(move || store.insert_chunk(&chunk))
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))?
    }

    async fn insert_chunks(&self, chunks: &[Chunk]) -> Result<()> {
        let store = self.store.clone();
        let chunks = chunks.to_vec();
        tokio::task::spawn_blocking(move || {
            for chunk in &chunks {
                store.insert_chunk(chunk)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::Internal(format!("Task join error: {}", e)))?
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        document_filter: Option<&[Uuid]>,
    ) -> Result<Vec<VectorSearchResult>> {
        let store = self.store.clone();
        let query = query_embedding.to_vec();
        let filter = document_filter.map(|f| f.to_vec());

        tokio::task::spawn_blocking(move || {
            let results = store.search(&query, top_k, filter.as_deref())?;
            Ok(results
                .into_iter()
                .map(|r| VectorSearchResult {
                    chunk: r.chunk,
                    similarity: r.similarity,
                })
                .collect())
        })
        .await
        .map_err(|e| Error::Internal(format!("Task join error: {}", e)))?
    }

    async fn delete_by_document(&self, document_id: &Uuid) -> Result<usize> {
        let store = self.store.clone();
        let doc_id = *document_id;
        tokio::task::spawn_blocking(move || store.delete_by_document(&doc_id))
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))?
    }

    async fn len(&self) -> Result<usize> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.len())
            .await
            .map_err(|e| Error::Internal(format!("Task join error: {}", e)))?
    }

    async fn health_check(&self) -> Result<bool> {
        // Local store is always healthy if it exists
        Ok(true)
    }

    fn name(&self) -> &str {
        "local-hnsw"
    }
}

/// Local document store using filesystem
pub struct LocalDocumentStore {
    /// Directory to store documents
    storage_dir: PathBuf,
}

impl LocalDocumentStore {
    /// Create a new local document store
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&storage_dir)?;
        Ok(Self { storage_dir })
    }

    /// Get path for a document
    fn doc_path(&self, doc_id: &Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.bin", doc_id))
    }

    /// Get metadata path for a document
    fn meta_path(&self, doc_id: &Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.meta.json", doc_id))
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentMeta {
    id: Uuid,
    filename: String,
    size: u64,
}

#[async_trait]
impl DocumentStoreProvider for LocalDocumentStore {
    async fn store_document(
        &self,
        doc_id: &Uuid,
        filename: &str,
        data: &[u8],
    ) -> Result<String> {
        let doc_path = self.doc_path(doc_id);
        let meta_path = self.meta_path(doc_id);

        // Write document data
        tokio::fs::write(&doc_path, data).await?;

        // Write metadata
        let meta = DocumentMeta {
            id: *doc_id,
            filename: filename.to_string(),
            size: data.len() as u64,
        };
        let meta_json = serde_json::to_string_pretty(&meta)?;
        tokio::fs::write(&meta_path, meta_json).await?;

        Ok(doc_path.to_string_lossy().to_string())
    }

    async fn get_document(&self, doc_id: &Uuid) -> Result<Vec<u8>> {
        let doc_path = self.doc_path(doc_id);
        tokio::fs::read(&doc_path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to read document {}: {}", doc_id, e)))
    }

    async fn exists(&self, doc_id: &Uuid) -> Result<bool> {
        let doc_path = self.doc_path(doc_id);
        Ok(doc_path.exists())
    }

    async fn delete_document(&self, doc_id: &Uuid) -> Result<()> {
        let doc_path = self.doc_path(doc_id);
        let meta_path = self.meta_path(doc_id);

        if doc_path.exists() {
            tokio::fs::remove_file(&doc_path).await?;
        }
        if meta_path.exists() {
            tokio::fs::remove_file(&meta_path).await?;
        }

        Ok(())
    }

    async fn list_documents(&self) -> Result<Vec<StoredDocumentInfo>> {
        let mut docs = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.storage_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(meta) = serde_json::from_str::<DocumentMeta>(&content) {
                        let doc_path = self.doc_path(&meta.id);
                        docs.push(StoredDocumentInfo {
                            id: meta.id,
                            filename: meta.filename,
                            uri: doc_path.to_string_lossy().to_string(),
                            size: meta.size,
                        });
                    }
                }
            }
        }

        Ok(docs)
    }

    async fn get_uri(&self, doc_id: &Uuid) -> Result<Option<String>> {
        let doc_path = self.doc_path(doc_id);
        if doc_path.exists() {
            Ok(Some(doc_path.to_string_lossy().to_string()))
        } else {
            Ok(None)
        }
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.storage_dir.exists())
    }

    fn name(&self) -> &str {
        "local-filesystem"
    }
}
