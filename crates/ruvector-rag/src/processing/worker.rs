//! Background worker for processing jobs

use futures_util::future::join_all;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};

use crate::error::Result;
use crate::ingestion::{ExternalParser, IngestPipeline};
use crate::server::state::{AppState, FileStatus};
use crate::types::Document;

use super::job_queue::{FileData, Job, JobQueue, JobStatus, ProcessingStage};

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
    parallel_files: usize,
    parallel_embeddings: usize,
}

impl ProcessingWorker {
    /// Create a new processing worker
    pub fn new(state: AppState, job_queue: Arc<JobQueue>) -> Self {
        let cpu_count = num_cpus::get();
        let parallel_files = cpu_count.min(8);      // Process up to 8 files in parallel
        let parallel_embeddings = cpu_count.min(4); // Embeddings per file

        tracing::info!(
            "Worker configured: {} parallel files, {} parallel embeddings per file",
            parallel_files,
            parallel_embeddings
        );

        Self {
            state,
            job_queue,
            parallel_files,
            parallel_embeddings,
        }
    }

    /// Start processing jobs from the queue
    pub async fn run(self, mut receiver: mpsc::Receiver<Job>) {
        tracing::info!(
            "Processing worker started: {} parallel files, {} embeddings/file",
            self.parallel_files,
            self.parallel_embeddings
        );

        while let Some(job) = receiver.recv().await {
            let job_id = job.id;
            tracing::info!("Processing job {} with {} files", job_id, job.files.len());

            self.job_queue.update_status(job_id, JobStatus::Processing, None);

            match self.process_job_parallel(job).await {
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

    /// Process a job with parallel file processing
    async fn process_job_parallel(&self, job: Job) -> Result<()> {
        let job_id = job.id;
        let parallel_embeddings = job.options.parallel_embeddings.max(1).min(self.parallel_embeddings);

        // Create semaphore to limit concurrent file processing
        let semaphore = Arc::new(Semaphore::new(self.parallel_files));

        // Create futures for all files
        let file_futures: Vec<_> = job.files.into_iter().map(|file_data| {
            let state = self.state.clone();
            let job_queue = self.job_queue.clone();
            let sem = semaphore.clone();
            let filename = file_data.filename.clone();

            async move {
                // Acquire semaphore permit
                let _permit = sem.acquire().await.unwrap();

                tracing::info!("Starting parallel processing: {}", filename);
                job_queue.update_current_file(job_id, &filename);

                // Process the file
                let result = Self::process_single_file(
                    &state,
                    &job_queue,
                    job_id,
                    file_data,
                    parallel_embeddings,
                ).await;

                (filename, result)
            }
        }).collect();

        // Wait for all files to complete
        let results = join_all(file_futures).await;

        // Process results
        for (filename, result) in results {
            match result {
                Ok(FileProcessResult::New(doc)) => {
                    self.state.add_document(doc);
                    self.job_queue.increment_files_processed(job_id);
                    tracing::info!("Processed new file: {}", filename);
                }
                Ok(FileProcessResult::Updated(doc, old_chunks)) => {
                    self.state.add_document(doc);
                    self.job_queue.increment_files_processed(job_id);
                    tracing::info!("Updated file: {}, deleted {} old chunks", filename, old_chunks);
                }
                Ok(FileProcessResult::Skipped(reason)) => {
                    tracing::info!("Skipped {}: {}", filename, reason);
                    self.job_queue.add_skipped_file(job_id, &filename, &reason);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    tracing::error!("Failed to process {}: {}", filename, error_msg);
                    self.job_queue.add_file_error(
                        job_id,
                        &filename,
                        &error_msg,
                        ProcessingStage::Parsing,
                    );
                }
            }
        }

        Ok(())
    }

    /// Process a single file (static method for parallel execution)
    async fn process_single_file(
        state: &AppState,
        job_queue: &Arc<JobQueue>,
        job_id: uuid::Uuid,
        file_data: FileData,
        parallel_embeddings: usize,
    ) -> Result<FileProcessResult> {
        let config = state.config();
        let external_parser = state.external_parser();
        let filename = &file_data.filename;
        let data = &file_data.data;

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
                    tracing::warn!("LibreOffice conversion failed for {}: {}, trying external API", filename, e);
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
        match state.check_file_status(&processed_filename, &parsed.content_hash) {
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
                let deleted = state.delete_document_with_chunks(&existing.id)?;
                tracing::info!(
                    "File '{}' modified, deleted {} old chunks",
                    processed_filename,
                    deleted
                );

                // Process the new version
                let doc = Self::process_file_content(
                    state,
                    job_queue,
                    job_id,
                    &processed_filename,
                    &processed_data,
                    &parsed,
                    parallel_embeddings,
                ).await?;
                return Ok(FileProcessResult::Updated(doc, deleted));
            }
            FileStatus::New => {
                // Process new file
                let doc = Self::process_file_content(
                    state,
                    job_queue,
                    job_id,
                    &processed_filename,
                    &processed_data,
                    &parsed,
                    parallel_embeddings,
                ).await?;
                return Ok(FileProcessResult::New(doc));
            }
        }
    }

    /// Process file content (chunking, embedding, storing)
    async fn process_file_content(
        state: &AppState,
        job_queue: &Arc<JobQueue>,
        job_id: uuid::Uuid,
        filename: &str,
        data: &[u8],
        parsed: &crate::ingestion::ParsedDocument,
        parallel_embeddings: usize,
    ) -> Result<Document> {
        let config = state.config();

        // Create pipeline
        let pipeline = IngestPipeline::new(
            config.chunking.chunk_size,
            config.chunking.chunk_overlap,
        );

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

        // Generate embeddings in parallel batches
        let chunk_batches: Vec<_> = chunks.chunks_mut(parallel_embeddings).collect();
        let ollama = state.ollama();

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
                        tracing::error!("Embedding failed for chunk in {}: {}", filename, e);
                        // Use zero vector as fallback
                        chunk.embedding = vec![0.0; config.embeddings.dimensions];
                    }
                }
            }

            job_queue.increment_chunks_embedded(job_id, batch.len());
        }

        // Store chunks
        let vector_store = state.vector_store();
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
