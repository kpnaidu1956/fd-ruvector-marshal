//! Background worker for processing jobs

use futures_util::future::join_all;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::ingestion::{ExternalParser, IngestPipeline};
use crate::server::state::{AppState, FileStatus};
use crate::types::Document;

use super::job_queue::{Job, JobQueue, JobStatus, ProcessingStage};

/// Result of processing a file
#[derive(Debug)]
pub enum FileProcessResult {
    /// New file processed
    New(Document),
    /// File was modified, reprocessed
    Updated(Document, usize),
    /// File was skipped (unchanged or duplicate)
    Skipped(String),
}

/// Worker for processing documents in the background
pub struct ProcessingWorker {
    state: AppState,
    job_queue: Arc<JobQueue>,
    parallel_embeddings: usize,
}

impl ProcessingWorker {
    /// Create a new processing worker
    pub fn new(state: AppState, job_queue: Arc<JobQueue>) -> Self {
        let parallel_embeddings = num_cpus::get().min(8);  // Max 8 parallel embeddings

        Self {
            state,
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

            // Process the file with deduplication
            match self.process_file_with_dedup(&file_data.filename, &file_data.data, job_id, parallel).await {
                Ok(FileProcessResult::New(doc)) => {
                    self.state.add_document(doc);
                    self.job_queue.increment_files_processed(job_id);
                }
                Ok(FileProcessResult::Updated(doc, old_chunks)) => {
                    self.state.add_document(doc);
                    self.job_queue.increment_files_processed(job_id);
                    tracing::info!("Updated file, deleted {} old chunks", old_chunks);
                }
                Ok(FileProcessResult::Skipped(reason)) => {
                    tracing::info!("Skipped {}: {}", file_data.filename, reason);
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

    /// Process a single file with deduplication check
    async fn process_file_with_dedup(
        &self,
        filename: &str,
        data: &[u8],
        job_id: uuid::Uuid,
        parallel: usize,
    ) -> Result<FileProcessResult> {
        // Stage: Parsing
        self.job_queue.update_stage(job_id, ProcessingStage::Parsing);

        let config = self.state.config();
        let external_parser = self.state.external_parser();

        // Check if we need to convert legacy format
        let (processed_filename, processed_data) = if ExternalParser::needs_conversion(filename) {
            match external_parser.convert_with_libreoffice(filename, data).await {
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
                    let parsed = external_parser.parse_with_unstructured(filename, data).await?;
                    (format!("{}.txt", filename), parsed.content.into_bytes())
                }
            }
        } else if ExternalParser::needs_external_parsing(filename) {
            let parsed = external_parser.parse_with_unstructured(filename, data).await?;
            (format!("{}.txt", filename), parsed.content.into_bytes())
        } else {
            (filename.to_string(), data.to_vec())
        };

        // Create pipeline
        let pipeline = IngestPipeline::new(
            config.chunking.chunk_size,
            config.chunking.chunk_overlap,
        );

        // Parse file to get content hash
        let parsed = pipeline.parse_file(&processed_filename, &processed_data)?;

        // Check file status for deduplication
        match self.state.check_file_status(&processed_filename, &parsed.content_hash) {
            FileStatus::Unchanged(existing) => {
                return Ok(FileProcessResult::Skipped(format!(
                    "unchanged (hash: {}...)",
                    &existing.content_hash[..12.min(existing.content_hash.len())]
                )));
            }
            FileStatus::Duplicate(existing) => {
                return Ok(FileProcessResult::Skipped(format!(
                    "duplicate of '{}'",
                    existing.filename
                )));
            }
            FileStatus::Modified(existing) => {
                // Delete old document and its chunks
                let deleted = self.state.delete_document_with_chunks(&existing.id)?;
                tracing::info!(
                    "File '{}' modified, deleted {} old chunks",
                    processed_filename,
                    deleted
                );

                // Process the new version
                let doc = self.process_file_internal(
                    &processed_filename,
                    &processed_data,
                    &parsed,
                    job_id,
                    parallel,
                ).await?;
                return Ok(FileProcessResult::Updated(doc, deleted));
            }
            FileStatus::New => {
                // Process new file
                let doc = self.process_file_internal(
                    &processed_filename,
                    &processed_data,
                    &parsed,
                    job_id,
                    parallel,
                ).await?;
                return Ok(FileProcessResult::New(doc));
            }
        }
    }

    /// Internal file processing (after dedup check)
    async fn process_file_internal(
        &self,
        filename: &str,
        data: &[u8],
        parsed: &crate::ingestion::ParsedDocument,
        job_id: uuid::Uuid,
        parallel: usize,
    ) -> Result<Document> {
        let config = self.state.config();

        // Create pipeline
        let pipeline = IngestPipeline::new(
            config.chunking.chunk_size,
            config.chunking.chunk_overlap,
        );

        // Stage: Chunking
        self.job_queue.update_stage(job_id, ProcessingStage::Chunking);

        // Create document
        let mut doc = Document::new(
            filename.to_string(),
            parsed.file_type.clone(),
            parsed.content_hash.clone(),
            data.len() as u64,
        );
        doc.total_pages = parsed.total_pages;

        // Create chunks
        let mut chunks = pipeline.create_chunks(&doc, parsed)?;
        let total_chunks = chunks.len();

        self.job_queue.set_total_chunks(job_id, total_chunks);

        // Stage: Embedding
        self.job_queue.update_stage(job_id, ProcessingStage::Embedding);

        // Generate embeddings in parallel batches
        let chunk_batches: Vec<_> = chunks.chunks_mut(parallel).collect();
        let ollama = self.state.ollama();

        for batch in chunk_batches {
            let embedding_futures: Vec<_> = batch
                .iter()
                .map(|chunk| ollama.embed(&chunk.content))
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
                        chunk.embedding = vec![0.0; config.embeddings.dimensions];
                    }
                }
            }

            self.job_queue.increment_chunks_embedded(job_id, batch.len());
        }

        // Stage: Storing
        self.job_queue.update_stage(job_id, ProcessingStage::Storing);

        // Store chunks
        let vector_store = self.state.vector_store();
        for chunk in &chunks {
            vector_store.insert_chunk(chunk)?;
        }

        doc.total_chunks = total_chunks as u32;

        tracing::info!(
            "Processed '{}': {} pages, {} chunks",
            filename,
            doc.total_pages.unwrap_or(1),
            total_chunks
        );

        Ok(doc)
    }
}
