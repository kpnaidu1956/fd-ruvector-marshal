//! Application state for the RAG server

use dashmap::DashMap;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::RagConfig;
use crate::error::Result;
use crate::generation::OllamaClient;
use crate::ingestion::ExternalParser;
use crate::learning::KnowledgeStore;
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
    /// External parser for legacy formats
    external_parser: ExternalParser,
    /// Knowledge store for learning
    knowledge_store: KnowledgeStore,
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

        // Initialize external parser for legacy formats
        let external_parser = ExternalParser::new(config.external_parser.clone());
        tracing::info!("External parser initialized (enabled: {})", config.external_parser.enabled);

        // Initialize knowledge store for learning
        let knowledge_path = config.vector_db.storage_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("knowledge.json");
        let knowledge_store = KnowledgeStore::new(knowledge_path);
        tracing::info!("Knowledge store initialized");

        Ok(Self {
            inner: Arc::new(AppStateInner {
                config,
                vector_store,
                ollama,
                external_parser,
                knowledge_store,
                documents: DashMap::new(),
                ready: RwLock::new(true),
            }),
        })
    }

    /// Get external parser
    pub fn external_parser(&self) -> &ExternalParser {
        &self.inner.external_parser
    }

    /// Get knowledge store
    pub fn knowledge_store(&self) -> &KnowledgeStore {
        &self.inner.knowledge_store
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
