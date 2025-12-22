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
use crate::processing::{JobQueue, ProcessingWorker};
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
    vector_store: Arc<VectorStore>,
    /// Ollama client (used for both embeddings and generation)
    ollama: Arc<OllamaClient>,
    /// External parser for legacy formats
    external_parser: Arc<ExternalParser>,
    /// Job queue for async processing
    job_queue: Arc<JobQueue>,
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
        let vector_store = Arc::new(VectorStore::new(&config)?);
        tracing::info!("Vector store initialized");

        // Initialize Ollama client (handles both embeddings and generation)
        let ollama = Arc::new(OllamaClient::new(&config.llm));
        tracing::info!("Ollama client initialized (using {} for embeddings)", config.llm.embed_model);

        // Initialize external parser for legacy formats
        let external_parser = Arc::new(ExternalParser::new(config.external_parser.clone()));
        tracing::info!("External parser initialized (enabled: {})", config.external_parser.enabled);

        // Initialize job queue and start workers
        let worker_count = num_cpus::get().min(4);  // Max 4 workers
        let (job_queue, receiver) = JobQueue::new(worker_count);
        let job_queue = Arc::new(job_queue);
        tracing::info!("Job queue initialized with {} workers", worker_count);

        // Start background worker
        let worker = ProcessingWorker::new(
            config.clone(),
            vector_store.clone(),
            ollama.clone(),
            external_parser.clone(),
            job_queue.clone(),
        );
        tokio::spawn(async move {
            worker.run(receiver).await;
        });

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
                job_queue,
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

    /// Get job queue
    pub fn job_queue(&self) -> &Arc<JobQueue> {
        &self.inner.job_queue
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
    pub fn vector_store(&self) -> &Arc<VectorStore> {
        &self.inner.vector_store
    }

    /// Get Ollama client (for embeddings and generation)
    pub fn ollama(&self) -> &Arc<OllamaClient> {
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
