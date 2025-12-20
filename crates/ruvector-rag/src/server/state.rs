//! Application state for the RAG server

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::RagConfig;
use crate::error::Result;
use crate::generation::OllamaClient;
use crate::retrieval::VectorStore;
use crate::types::Document;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    /// Configuration
    config: RagConfig,
    /// Vector store for chunks
    vector_store: VectorStore,
    /// Ollama client (used for both embeddings and generation)
    ollama: OllamaClient,
    /// Document registry (in-memory for now)
    documents: DashMap<Uuid, Document>,
    /// Ready state
    ready: RwLock<bool>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: RagConfig) -> Result<Self> {
        tracing::info!("Initializing RAG application state...");

        // Initialize vector store
        let vector_store = VectorStore::new(&config)?;
        tracing::info!("Vector store initialized");

        // Initialize Ollama client (handles both embeddings and generation)
        let ollama = OllamaClient::new(&config.llm);
        tracing::info!("Ollama client initialized (using {} for embeddings)", config.llm.embed_model);

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                vector_store,
                ollama,
                documents: DashMap::new(),
                ready: RwLock::new(true),
            }),
        })
    }

    /// Get configuration
    pub fn config(&self) -> &RagConfig {
        &self.inner.config
    }

    /// Get vector store
    pub fn vector_store(&self) -> &VectorStore {
        &self.inner.vector_store
    }

    /// Get Ollama client (for embeddings and generation)
    pub fn ollama(&self) -> &OllamaClient {
        &self.inner.ollama
    }

    /// Get documents map
    pub fn documents(&self) -> &DashMap<Uuid, Document> {
        &self.inner.documents
    }

    /// Check if the server is ready
    pub fn is_ready(&self) -> bool {
        *self.inner.ready.read()
    }

    /// Set ready state
    pub fn set_ready(&self, ready: bool) {
        *self.inner.ready.write() = ready;
    }

    /// Add a document to the registry
    pub fn add_document(&self, doc: Document) {
        self.inner.documents.insert(doc.id, doc);
    }

    /// Get a document by ID
    pub fn get_document(&self, id: &Uuid) -> Option<Document> {
        self.inner.documents.get(id).map(|d| d.clone())
    }

    /// Remove a document
    pub fn remove_document(&self, id: &Uuid) -> Option<Document> {
        self.inner.documents.remove(id).map(|(_, d)| d)
    }

    /// List all documents
    pub fn list_documents(&self) -> Vec<Document> {
        self.inner
            .documents
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}
