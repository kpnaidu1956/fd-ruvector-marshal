//! Application state for the RAG server

use dashmap::DashMap;
use parking_lot::RwLock;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::{BackendProvider, RagConfig};
use crate::error::{Error, Result};
use crate::generation::OllamaClient;
use crate::ingestion::ExternalParser;
use crate::learning::{AnswerCache, KnowledgeStore};
use crate::processing::{JobQueue, ProcessingWorker};
use crate::providers::{
    EmbeddingProvider, LlmProvider, VectorStoreProvider,
    local::LocalVectorStore,
    ollama::{OllamaEmbedder, OllamaLlm},
};
#[cfg(feature = "gcp")]
use crate::providers::gcp::{DocumentAiClient, GcsDocumentStore};
use crate::retrieval::VectorStore;
use crate::storage::{FileRegistryDb, FileRegistryDbStats, SyncStatus};
use crate::types::{Chunk, Document, FileRecord, FileRecordStatus, SkipReason};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    /// Configuration
    config: RagConfig,
    /// Vector store for chunks (provider abstraction)
    vector_store_provider: Arc<dyn VectorStoreProvider>,
    /// Legacy vector store (for backwards compatibility)
    vector_store: Arc<VectorStore>,
    /// Embedding provider (Ollama or Vertex AI)
    embedding_provider: Arc<dyn EmbeddingProvider>,
    /// LLM provider (Ollama or Gemini)
    llm_provider: Arc<dyn LlmProvider>,
    /// Ollama client (legacy, for backwards compatibility)
    ollama: Arc<OllamaClient>,
    /// External parser for legacy formats
    external_parser: Arc<ExternalParser>,
    /// Job queue for async processing
    job_queue: Arc<JobQueue>,
    /// Knowledge store for learning
    knowledge_store: KnowledgeStore,
    /// Answer cache with document-based invalidation
    answer_cache: AnswerCache,
    /// Document registry (in-memory cache, backed by database)
    documents: DashMap<Uuid, Document>,
    /// Chunk metadata store (for Vertex AI lookups)
    chunks: DashMap<Uuid, Chunk>,
    /// File registry (in-memory cache for fast lookups)
    file_registry: DashMap<String, FileRecord>,
    /// SQLite database for persistent storage
    database: Arc<FileRegistryDb>,
    /// Path for documents JSON (legacy, for backwards compatibility)
    documents_path: PathBuf,
    /// Ready state
    ready: RwLock<bool>,
    /// GCS document store (only for GCP backend)
    #[cfg(feature = "gcp")]
    document_store: Option<Arc<GcsDocumentStore>>,
    /// Document AI client for advanced PDF extraction (only for GCP backend)
    #[cfg(feature = "gcp")]
    document_ai: Option<Arc<DocumentAiClient>>,
}

impl AppState {
    /// Create new application state
    pub async fn new(config: RagConfig) -> Result<Self> {
        tracing::info!("Initializing RAG application state (backend: {:?})...", config.backend);

        // Initialize vector store (always local for now)
        let vector_store = Arc::new(VectorStore::new(&config)?);
        tracing::info!("Vector store initialized");

        // Initialize Ollama client (for local backend, also used as fallback)
        let ollama = Arc::new(OllamaClient::new(&config.llm));
        tracing::info!("Ollama client initialized (using {} for embeddings)", config.llm.embed_model);

        // Initialize document store (GCP only)
        #[cfg(feature = "gcp")]
        let mut gcs_document_store: Option<Arc<GcsDocumentStore>> = None;
        #[cfg(feature = "gcp")]
        let mut document_ai_client: Option<Arc<DocumentAiClient>> = None;

        // Initialize providers based on backend
        let (embedding_provider, llm_provider, vector_store_provider): (
            Arc<dyn EmbeddingProvider>,
            Arc<dyn LlmProvider>,
            Arc<dyn VectorStoreProvider>,
        ) = match config.backend {
            BackendProvider::Local => {
                tracing::info!("Using local backend (Ollama + HNSW)");
                let embedder = Arc::new(OllamaEmbedder::new(
                    &config.llm,
                    config.embeddings.dimensions,
                ));
                let llm = Arc::new(OllamaLlm::new(&config.llm));
                let vector_provider = Arc::new(LocalVectorStore::new(Arc::clone(&vector_store)));
                (embedder, llm, vector_provider)
            }
            BackendProvider::Gcp => {
                #[cfg(feature = "gcp")]
                {
                    use crate::providers::gcp::{GcpAuth, GeminiClient, VertexAiEmbedder, VertexVectorSearch};

                    let gcp_config = config.gcp.as_ref().ok_or_else(|| {
                        Error::Config("GCP backend selected but gcp config is missing".to_string())
                    })?;

                    tracing::info!("Using GCP backend (Vertex AI + Gemini)");

                    let auth = Arc::new(GcpAuth::from_service_account(
                        &gcp_config.service_account_key_path,
                        gcp_config.project_id.clone(),
                    )?);

                    let embedder = Arc::new(VertexAiEmbedder::new(
                        Arc::clone(&auth),
                        gcp_config.location.clone(),
                        Some(gcp_config.embedding_model.clone()),
                    ));

                    let llm = Arc::new(GeminiClient::new(
                        Arc::clone(&auth),
                        gcp_config.location.clone(),
                        Some(gcp_config.generation_model.clone()),
                    ));

                    let vector_provider = Arc::new(VertexVectorSearch::new(
                        Arc::clone(&auth),
                        gcp_config.location.clone(),
                        gcp_config.vector_search_index.clone(),
                        gcp_config.vector_search_endpoint.clone(),
                        gcp_config.vector_search_public_domain.clone(),
                        gcp_config.deployed_index_id.clone(),
                    ));

                    // Initialize GCS document store
                    let document_store = GcsDocumentStore::new(
                        Arc::clone(&auth),
                        gcp_config.gcs_bucket.clone(),
                        Some(gcp_config.gcs_originals_prefix.clone()),
                        Some(gcp_config.gcs_plaintext_prefix.clone()),
                    ).await?;
                    gcs_document_store = Some(Arc::new(document_store));

                    // Initialize Document AI client if processor is configured
                    if let Some(ref processor_name) = gcp_config.document_ai_processor {
                        if gcp_config.use_document_ai_fallback {
                            let doc_ai = DocumentAiClient::new(
                                Arc::clone(&auth),
                                processor_name.clone(),
                            );
                            document_ai_client = Some(Arc::new(doc_ai));
                            tracing::info!(
                                "Document AI initialized (processor: {})",
                                processor_name
                            );
                        }
                    }

                    tracing::info!(
                        "GCP providers initialized (embedding: {}, llm: {}, gcs: {}, document_ai: {})",
                        gcp_config.embedding_model,
                        gcp_config.generation_model,
                        gcp_config.gcs_bucket,
                        if document_ai_client.is_some() { "enabled" } else { "disabled" }
                    );

                    (embedder, llm, vector_provider)
                }
                #[cfg(not(feature = "gcp"))]
                {
                    return Err(Error::Config(
                        "GCP backend selected but gcp feature is not enabled. \
                         Rebuild with --features gcp".to_string()
                    ));
                }
            }
        };

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

        // Initialize answer cache (1000 entries, 1 hour TTL)
        let answer_cache = AnswerCache::new(1000, 3600);
        tracing::info!("Answer cache initialized");

        // Initialize SQLite database
        let db_path = storage_dir.join("rag_registry.db");
        let database = Arc::new(FileRegistryDb::new(&db_path)?);
        tracing::info!("Database initialized at {:?}", db_path);

        // Load file registry from database into memory cache
        let file_registry = DashMap::new();
        match database.list_file_records() {
            Ok(records) => {
                for record in records {
                    file_registry.insert(record.filename.clone(), record);
                }
                tracing::info!("Loaded {} file records from database", file_registry.len());
            }
            Err(e) => {
                tracing::warn!("Failed to load file registry from database: {}", e);
            }
        }

        // Load documents from legacy JSON file if database is empty
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
                vector_store_provider,
                vector_store,
                embedding_provider,
                llm_provider,
                ollama,
                external_parser,
                job_queue: job_queue.clone(),
                knowledge_store,
                answer_cache,
                documents,
                chunks: DashMap::new(),
                file_registry,
                database,
                documents_path,
                ready: RwLock::new(true),
                #[cfg(feature = "gcp")]
                document_store: gcs_document_store,
                #[cfg(feature = "gcp")]
                document_ai: document_ai_client,
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

    /// Get database reference
    pub fn database(&self) -> &Arc<FileRegistryDb> {
        &self.inner.database
    }

    /// Sync file registry from GCS bucket
    /// Returns (files_synced, failed_count)
    #[cfg(feature = "gcp")]
    pub async fn sync_from_gcs(&self) -> Result<(usize, usize)> {
        let document_store = self.document_store()
            .ok_or_else(|| Error::Internal("GCS document store not available".to_string()))?;

        let start = std::time::Instant::now();
        let files = document_store.sync_from_bucket().await?;

        let mut synced = 0;
        let mut failed = 0;

        for file_info in &files {
            // Update database
            if let Err(e) = self.inner.database.sync_from_gcs(
                &file_info.filename,
                file_info.document_id,
                file_info.content_hash.as_deref().unwrap_or(""),
                file_info.file_size,
                &file_info.file_type,
                file_info.has_plaintext,
                &file_info.original_uri,
                file_info.plaintext_uri.as_deref(),
            ) {
                tracing::warn!("Failed to sync file {}: {}", file_info.filename, e);
                failed += 1;
                continue;
            }

            // Update in-memory cache
            let status = if file_info.has_plaintext {
                FileRecordStatus::Success
            } else {
                FileRecordStatus::Failed
            };

            let record = FileRecord {
                id: file_info.document_id,
                filename: file_info.filename.clone(),
                content_hash: file_info.content_hash.clone().unwrap_or_default(),
                file_size: file_info.file_size,
                file_type: crate::types::FileType::from_extension(&file_info.file_type),
                status,
                document_id: Some(file_info.document_id),
                chunks_created: None,
                skip_reason: None,
                error_message: if file_info.has_plaintext { None } else {
                    Some("No plaintext found - processing may have failed".to_string())
                },
                failed_at_stage: None,
                job_id: None,
                first_seen_at: chrono::Utc::now(),
                last_processed_at: chrono::Utc::now(),
                upload_count: 1,
                original_url: Some(file_info.original_uri.clone()),
                plaintext_url: file_info.plaintext_uri.clone(),
            };

            self.inner.file_registry.insert(file_info.filename.clone(), record);
            synced += 1;
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        if let Err(e) = self.inner.database.update_sync_status(synced, duration_ms) {
            tracing::warn!("Failed to update sync status: {}", e);
        }

        tracing::info!(
            "GCS sync complete: {} files synced, {} failed, took {}ms",
            synced, failed, duration_ms
        );

        Ok((synced, failed))
    }

    /// Get GCS sync status
    pub fn get_sync_status(&self) -> Option<SyncStatus> {
        self.inner.database.get_sync_status().ok().flatten()
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

    /// Get answer cache
    pub fn answer_cache(&self) -> &AnswerCache {
        &self.inner.answer_cache
    }

    /// Get document timestamps for cache validation
    pub fn get_document_timestamps(&self) -> std::collections::HashMap<Uuid, chrono::DateTime<chrono::Utc>> {
        self.inner
            .documents
            .iter()
            .map(|entry| (*entry.key(), entry.value().ingested_at))
            .collect()
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
    /// NOTE: Prefer using embedding_provider() and llm_provider() for new code
    pub fn ollama(&self) -> &Arc<OllamaClient> {
        &self.inner.ollama
    }

    /// Get embedding provider (Ollama or Vertex AI based on config)
    pub fn embedding_provider(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.inner.embedding_provider
    }

    /// Get LLM provider (Ollama or Gemini based on config)
    pub fn llm_provider(&self) -> &Arc<dyn LlmProvider> {
        &self.inner.llm_provider
    }

    /// Get vector store provider (Local HNSW or Vertex AI Vector Search)
    pub fn vector_store_provider(&self) -> &Arc<dyn VectorStoreProvider> {
        &self.inner.vector_store_provider
    }

    /// Get GCS document store (only available with GCP backend)
    #[cfg(feature = "gcp")]
    pub fn document_store(&self) -> Option<&Arc<GcsDocumentStore>> {
        self.inner.document_store.as_ref()
    }

    /// Get Document AI client (only available with GCP backend and processor configured)
    #[cfg(feature = "gcp")]
    pub fn document_ai(&self) -> Option<&Arc<DocumentAiClient>> {
        self.inner.document_ai.as_ref()
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

    /// Store chunks in the local chunk store (for Vertex AI metadata lookup)
    pub fn store_chunks(&self, chunks: &[Chunk]) {
        for chunk in chunks {
            self.inner.chunks.insert(chunk.id, chunk.clone());
        }
    }

    /// Get a chunk by ID from the local store
    pub fn get_chunk(&self, id: &Uuid) -> Option<Chunk> {
        self.inner.chunks.get(id).map(|c| c.clone())
    }

    /// Check if file should be processed (returns action to take)
    /// Returns: (should_process, existing_doc_to_delete)
    pub fn check_file_status(&self, filename: &str, content_hash: &str) -> FileStatus {
        // First, check file registry (includes files synced from GCS)
        if let Some(record) = self.inner.file_registry.get(filename) {
            if record.content_hash == content_hash && record.status == FileRecordStatus::Success {
                // Same file, same content, already successfully processed - skip
                tracing::info!(
                    "File '{}' already exists in registry (hash: {}..., synced from GCS)",
                    filename,
                    &content_hash[..content_hash.len().min(12)]
                );
                return FileStatus::ExistsInRegistry(record.clone());
            }
        }

        // Check by content hash in file registry
        if let Some(record) = self.get_file_record_by_hash(content_hash) {
            if record.status == FileRecordStatus::Success {
                tracing::info!(
                    "File with same content already exists as '{}' (hash: {}...)",
                    record.filename,
                    &content_hash[..content_hash.len().min(12)]
                );
                return FileStatus::DuplicateInRegistry(record);
            }
        }

        // Check if exact same content exists in documents (by hash)
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
        // Invalidate cached answers that cite this document
        self.inner.answer_cache.invalidate_by_document(doc_id);

        // Delete chunks from vector store
        let deleted = self.inner.vector_store.delete_by_document(doc_id)?;

        // Remove from document registry
        self.inner.documents.remove(doc_id);

        Ok(deleted)
    }

    // ==================== File Registry Methods ====================

    /// Record a successful file processing
    pub fn record_file_success(
        &self,
        filename: &str,
        content_hash: &str,
        file_size: u64,
        file_type: crate::types::FileType,
        document_id: Uuid,
        chunks_created: u32,
        job_id: Option<Uuid>,
    ) {
        let record = FileRecord::success(
            filename.to_string(),
            content_hash.to_string(),
            file_size,
            file_type,
            document_id,
            chunks_created,
            job_id,
        );
        // Save to database
        if let Err(e) = self.inner.database.upsert_file_record(&record) {
            tracing::error!("Failed to save file record to database: {}", e);
        }
        // Update in-memory cache
        self.inner.file_registry.insert(filename.to_string(), record);
    }

    /// Record a skipped file
    pub fn record_file_skipped(
        &self,
        filename: &str,
        content_hash: &str,
        file_size: u64,
        file_type: crate::types::FileType,
        skip_reason: SkipReason,
        job_id: Option<Uuid>,
    ) {
        let record = if let Some(mut existing) = self.inner.file_registry.get_mut(filename) {
            existing.update_for_reupload(job_id);
            existing.status = FileRecordStatus::Skipped;
            existing.skip_reason = Some(skip_reason);
            existing.clone()
        } else {
            FileRecord::skipped(
                filename.to_string(),
                content_hash.to_string(),
                file_size,
                file_type,
                skip_reason,
                job_id,
            )
        };
        // Save to database
        if let Err(e) = self.inner.database.upsert_file_record(&record) {
            tracing::error!("Failed to save file record to database: {}", e);
        }
        // Update in-memory cache
        self.inner.file_registry.insert(filename.to_string(), record);
    }

    /// Record a failed file
    pub fn record_file_failed(
        &self,
        filename: &str,
        content_hash: &str,
        file_size: u64,
        file_type: crate::types::FileType,
        error_message: &str,
        failed_at_stage: &str,
        job_id: Option<Uuid>,
    ) {
        let record = if let Some(mut existing) = self.inner.file_registry.get_mut(filename) {
            existing.update_for_reupload(job_id);
            existing.mark_failed(error_message.to_string(), failed_at_stage.to_string());
            existing.clone()
        } else {
            FileRecord::failed(
                filename.to_string(),
                content_hash.to_string(),
                file_size,
                file_type,
                error_message.to_string(),
                failed_at_stage.to_string(),
                job_id,
            )
        };
        // Save to database
        if let Err(e) = self.inner.database.upsert_file_record(&record) {
            tracing::error!("Failed to save file record to database: {}", e);
        }
        // Update in-memory cache
        self.inner.file_registry.insert(filename.to_string(), record);
    }

    /// Get file record by filename
    pub fn get_file_record(&self, filename: &str) -> Option<FileRecord> {
        self.inner.file_registry.get(filename).map(|r| r.clone())
    }

    /// Get file record by content hash
    pub fn get_file_record_by_hash(&self, content_hash: &str) -> Option<FileRecord> {
        self.inner
            .file_registry
            .iter()
            .find(|entry| entry.value().content_hash == content_hash)
            .map(|entry| entry.value().clone())
    }

    /// List all file records
    pub fn list_file_records(&self) -> Vec<FileRecord> {
        self.inner
            .file_registry
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List successful file records
    pub fn list_successful_files(&self) -> Vec<FileRecord> {
        self.inner
            .file_registry
            .iter()
            .filter(|entry| entry.value().status == FileRecordStatus::Success)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List failed file records
    pub fn list_failed_files(&self) -> Vec<FileRecord> {
        self.inner
            .file_registry
            .iter()
            .filter(|entry| entry.value().status == FileRecordStatus::Failed)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List skipped file records
    pub fn list_skipped_files(&self) -> Vec<FileRecord> {
        self.inner
            .file_registry
            .iter()
            .filter(|entry| entry.value().status == FileRecordStatus::Skipped)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get file registry statistics
    pub fn file_registry_stats(&self) -> FileRegistryStats {
        let total = self.inner.file_registry.len();
        let success = self.inner.file_registry.iter()
            .filter(|e| e.value().status == FileRecordStatus::Success)
            .count();
        let failed = self.inner.file_registry.iter()
            .filter(|e| e.value().status == FileRecordStatus::Failed)
            .count();
        let skipped = self.inner.file_registry.iter()
            .filter(|e| e.value().status == FileRecordStatus::Skipped)
            .count();

        FileRegistryStats { total, success, failed, skipped }
    }

    /// Remove a file record
    pub fn remove_file_record(&self, filename: &str) -> Option<FileRecord> {
        // Remove from database
        if let Err(e) = self.inner.database.delete_file_record(filename) {
            tracing::error!("Failed to delete file record from database: {}", e);
        }
        // Remove from in-memory cache
        self.inner.file_registry.remove(filename).map(|(_, r)| r)
    }

    /// Clear all failed file records (for retry)
    pub fn clear_failed_files(&self) -> usize {
        // Clear from database
        let db_count = match self.inner.database.clear_failed_files() {
            Ok(count) => count,
            Err(e) => {
                tracing::error!("Failed to clear failed files from database: {}", e);
                0
            }
        };

        // Clear from in-memory cache
        let failed_keys: Vec<String> = self.inner
            .file_registry
            .iter()
            .filter(|e| e.value().status == FileRecordStatus::Failed)
            .map(|e| e.key().clone())
            .collect();

        for key in &failed_keys {
            self.inner.file_registry.remove(key);
        }

        db_count.max(failed_keys.len())
    }

    /// Get database statistics
    pub fn database_stats(&self) -> FileRegistryDbStats {
        self.inner.database.get_stats().unwrap_or(FileRegistryDbStats {
            total: 0,
            success: 0,
            failed: 0,
            skipped: 0,
        })
    }
}

/// Statistics for file registry
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRegistryStats {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped: usize,
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
    /// File already exists in registry (synced from GCS) - skip
    ExistsInRegistry(FileRecord),
    /// Same content exists in registry under different filename - skip
    DuplicateInRegistry(FileRecord),
}
