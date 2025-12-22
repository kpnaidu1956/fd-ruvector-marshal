//! Background worker for processing jobs

use futures_util::future::join_all;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::RagConfig;
use crate::error::Result;
use crate::generation::OllamaClient;
use crate::ingestion::{ExternalParser, IngestPipeline};
use crate::retrieval::VectorStore;
use crate::types::{Chunk, Document};

use super::job_queue::{Job, JobQueue, JobStatus, ProcessingStage};

/// Worker for processing documents in the background
pub struct ProcessingWorker {
    config: RagConfig,
    vector_store: Arc<VectorStore>,
    ollama: Arc<OllamaClient>,
    external_parser: Arc<ExternalParser>,
    job_queue: Arc<JobQueue>,
    parallel_embeddings: usize,
}

impl ProcessingWorker {
    /// Create a new processing worker
    pub fn new(
        config: RagConfig,
        vector_store: Arc<VectorStore>,
        ollama: Arc<OllamaClient>,
        external_parser: Arc<ExternalParser>,
        job_queue: Arc<JobQueue>,
    ) -> Self {
        let parallel_embeddings = num_cpus::get().min(8);  // Max 8 parallel embeddings

        Self {
            config,
            vector_store,
            ollama,
            external_parser,
            job_queue,
            parallel_embeddings,
        }
    }

    /// Start processing jobs from the queue
    pub async fn run(self, mut receiver: mpsc::Receiver<Job>) {
        tracing::info!("Processing worker started with {} parallel embeddings", self.parallel_embeddings);

        while let Some(job) = receiver.recv().await {
            let job_id = job.id;
            tracing::info!("Processing job {} with {} files", job_id, job.files.len());

            self.job_queue.update_status(job_id, JobStatus::Processing, None);

            match self.process_job(job).await {
                Ok(()) => {
                    self.job_queue.update_stage(job_id, ProcessingStage::Complete);
                    tracing::info!("Job {} completed successfully", job_id);
                }
                Err(e) => {
                    self.job_queue.update_status(job_id, JobStatus::Failed, Some(e.to_string()));
                    tracing::error!("Job {} failed: {}", job_id, e);
                }
            }
        }
    }

    /// Process a single job
    async fn process_job(&self, job: Job) -> Result<()> {
        let job_id = job.id;
        let parallel = job.options.parallel_embeddings.max(1).min(self.parallel_embeddings);

        for file_data in job.files {
            self.job_queue.update_current_file(job_id, &file_data.filename);

            // Process the file
            match self.process_file(&file_data.filename, &file_data.data, job_id, parallel).await {
                Ok(_) => {
                    self.job_queue.increment_files_processed(job_id);
                }
                Err(e) => {
                    tracing::error!("Failed to process {}: {}", file_data.filename, e);
                    // Continue with other files
                }
            }
        }

        Ok(())
    }

    /// Process a single file
    async fn process_file(
        &self,
        filename: &str,
        data: &[u8],
        job_id: uuid::Uuid,
        parallel: usize,
    ) -> Result<Document> {
        // Stage: Parsing
        self.job_queue.update_stage(job_id, ProcessingStage::Parsing);

        // Check if we need to convert legacy format
        let (processed_filename, processed_data) = if ExternalParser::needs_conversion(filename) {
            match self.external_parser.convert_with_libreoffice(filename, data).await {
                Ok(converted) => {
                    let stem = std::path::Path::new(filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("document");
                    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
                    let new_ext = match ext.as_str() {
                        "doc" => "docx",
                        "ppt" => "pptx",
                        "xls" => "xlsx",
                        _ => "docx",
                    };
                    (format!("{}.{}", stem, new_ext), converted)
                }
                Err(e) => {
                    tracing::warn!("LibreOffice conversion failed: {}, trying external API", e);
                    let parsed = self.external_parser.parse_with_unstructured(filename, data).await?;
                    (format!("{}.txt", filename), parsed.content.into_bytes())
                }
            }
        } else if ExternalParser::needs_external_parsing(filename) {
            let parsed = self.external_parser.parse_with_unstructured(filename, data).await?;
            (format!("{}.txt", filename), parsed.content.into_bytes())
        } else {
            (filename.to_string(), data.to_vec())
        };

        // Create pipeline
        let pipeline = IngestPipeline::new(
            self.config.chunking.chunk_size,
            self.config.chunking.chunk_overlap,
        );

        // Parse file
        let parsed = pipeline.parse_file(&processed_filename, &processed_data)?;

        // Stage: Chunking
        self.job_queue.update_stage(job_id, ProcessingStage::Chunking);

        // Create document
        let mut doc = Document::new(
            processed_filename.clone(),
            parsed.file_type.clone(),
            parsed.content_hash.clone(),
            processed_data.len() as u64,
        );
        doc.total_pages = parsed.total_pages;

        // Create chunks
        let mut chunks = pipeline.create_chunks(&doc, &parsed)?;
        let total_chunks = chunks.len();

        self.job_queue.set_total_chunks(job_id, total_chunks);

        // Stage: Embedding
        self.job_queue.update_stage(job_id, ProcessingStage::Embedding);

        // Generate embeddings in parallel batches
        let chunk_batches: Vec<_> = chunks.chunks_mut(parallel).collect();

        for batch in chunk_batches {
            let embedding_futures: Vec<_> = batch
                .iter()
                .map(|chunk| self.ollama.embed(&chunk.content))
                .collect();

            let results = join_all(embedding_futures).await;

            for (chunk, result) in batch.iter_mut().zip(results) {
                match result {
                    Ok(embedding) => {
                        chunk.embedding = embedding;
                    }
                    Err(e) => {
                        tracing::error!("Embedding failed for chunk: {}", e);
                        // Use zero vector as fallback
                        chunk.embedding = vec![0.0; self.config.embeddings.dimensions];
                    }
                }
            }

            self.job_queue.increment_chunks_embedded(job_id, batch.len());
        }

        // Stage: Storing
        self.job_queue.update_stage(job_id, ProcessingStage::Storing);

        // Store chunks
        for chunk in &chunks {
            self.vector_store.insert_chunk(chunk)?;
        }

        doc.total_chunks = total_chunks as u32;

        tracing::info!(
            "Processed '{}': {} pages, {} chunks",
            processed_filename,
            doc.total_pages.unwrap_or(1),
            total_chunks
        );

        Ok(doc)
    }
}
