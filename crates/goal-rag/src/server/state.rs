//! Application state for the RAG server

use dashmap::DashMap;
use parking_lot::RwLock;
use std::fs;
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
    /// Document registry (persisted to disk)
    documents: DashMap<Uuid, Document>,
    /// Path to documents registry file
    documents_path: PathBuf,
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

        // Initialize knowledge store for learning
        let storage_dir = config.vector_db.storage_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let knowledge_path = storage_dir.join("knowledge.json");
        let knowledge_store = KnowledgeStore::new(knowledge_path);
        tracing::info!("Knowledge store initialized");

        // Load persisted documents registry
        let documents_path = storage_dir.join("documents.json");
        let documents = Self::load_documents(&documents_path);
        tracing::info!("Loaded {} documents from registry", documents.len());

        // Initialize job queue and start workers
        let worker_count = num_cpus::get().min(4);  // Max 4 workers
        let (job_queue, receiver) = JobQueue::new(worker_count);
        let job_queue = Arc::new(job_queue);
        tracing::info!("Job queue initialized with {} workers", worker_count);

        // Create the state first (without the worker running)
        let state = Self {
            inner: Arc::new(AppStateInner {
                config,
                vector_store,
                ollama,
                external_parser,
                job_queue: job_queue.clone(),
                knowledge_store,
                documents,
                documents_path,
                ready: RwLock::new(true),
            }),
        };

        // Start background worker with a clone of the state
        let worker_state = state.clone();
        let worker = ProcessingWorker::new(worker_state, job_queue);
        tokio::spawn(async move {
            worker.run(receiver).await;
        });

        Ok(state)
    }

    /// Load documents from disk
    fn load_documents(path: &PathBuf) -> DashMap<Uuid, Document> {
        let documents = DashMap::new();

        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<Vec<Document>>(&content) {
                        Ok(docs) => {
                            for doc in docs {
                                documents.insert(doc.id, doc);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse documents.json: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read documents.json: {}", e);
                }
            }
        }

        documents
    }

    /// Save documents to disk
    fn save_documents(&self) {
        let docs: Vec<Document> = self.inner.documents
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        match serde_json::to_string_pretty(&docs) {
            Ok(content) => {
                if let Err(e) = fs::write(&self.inner.documents_path, content) {
                    tracing::error!("Failed to save documents.json: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize documents: {}", e);
            }
        }
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

    /// Add a document to the registry (persisted to disk)
    pub fn add_document(&self, doc: Document) {
        self.inner.documents.insert(doc.id, doc);
        self.save_documents();
    }

    /// Get a document by ID
    pub fn get_document(&self, id: &Uuid) -> Option<Document> {
        self.inner.documents.get(id).map(|d| d.clone())
    }

    /// Remove a document (persisted to disk)
    pub fn remove_document(&self, id: &Uuid) -> Option<Document> {
        let removed = self.inner.documents.remove(id).map(|(_, d)| d);
        if removed.is_some() {
            self.save_documents();
        }
        removed
    }

    /// List all documents
    pub fn list_documents(&self) -> Vec<Document> {
        self.inner
            .documents
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Find document by filename
    pub fn find_by_filename(&self, filename: &str) -> Option<Document> {
        self.inner
            .documents
            .iter()
            .find(|entry| entry.value().filename == filename)
            .map(|entry| entry.value().clone())
    }

    /// Find document by content hash
    pub fn find_by_hash(&self, content_hash: &str) -> Option<Document> {
        self.inner
            .documents
            .iter()
            .find(|entry| entry.value().content_hash == content_hash)
            .map(|entry| entry.value().clone())
    }

    /// Check if file should be processed (returns action to take)
    /// Returns: (should_process, existing_doc_to_delete)
    pub fn check_file_status(&self, filename: &str, content_hash: &str) -> FileStatus {
        // First, check if exact same content exists (by hash)
        if let Some(existing) = self.find_by_hash(content_hash) {
            if existing.filename == filename {
                // Same file, same content - skip
                return FileStatus::Unchanged(existing);
            } else {
                // Different filename, same content - could be a rename or duplicate
                // We'll still skip since content is identical
                return FileStatus::Duplicate(existing);
            }
        }

        // Check if file with same name exists but different content
        if let Some(existing) = self.find_by_filename(filename) {
            // Same filename, different content - file was modified
            return FileStatus::Modified(existing);
        }

        // New file
        FileStatus::New
    }

    /// Delete document and its chunks
    pub fn delete_document_with_chunks(&self, doc_id: &Uuid) -> crate::error::Result<usize> {
        // Delete chunks from vector store
        let deleted = self.inner.vector_store.delete_by_document(doc_id)?;

        // Remove from document registry
        self.inner.documents.remove(doc_id);

        Ok(deleted)
    }
}

/// Status of a file for deduplication
#[derive(Debug, Clone)]
pub enum FileStatus {
    /// File is new, process it
    New,
    /// File exists with same content - skip processing
    Unchanged(Document),
    /// Same content exists under different filename - skip
    Duplicate(Document),
    /// File exists but content changed - delete old and reprocess
    Modified(Document),
}
