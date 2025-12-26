//! Background worker for processing jobs

use futures_util::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::timeout;

use crate::error::{Error, Result};
use crate::ingestion::{ExternalParser, IngestPipeline};
#[cfg(feature = "gcp")]
use crate::providers::document_store::DocumentStoreProvider;
use crate::server::state::{AppState, FileStatus};
use crate::types::{Document, FileType, SkipReason};

use super::job_queue::{FileData, Job, JobQueue, JobStatus, ProcessingStage};

/// Result of processing a file
#[derive(Debug)]
pub enum FileProcessResult {
    /// New file processed
    New {
        document: Document,
        file_size: u64,
    },
    /// File was modified, reprocessed
    Updated {
        document: Document,
        file_size: u64,
        old_chunks_deleted: usize,
    },
    /// File was skipped (unchanged or duplicate)
    Skipped {
        reason: String,
        skip_reason: SkipReason,
        content_hash: String,
        file_size: u64,
        file_type: FileType,
    },
}

/// Worker for processing documents in the background
pub struct ProcessingWorker {
    state: AppState,
    job_queue: Arc<JobQueue>,
    parallel_files: usize,
    parallel_embeddings: usize,
    file_timeout: Duration,
}

impl ProcessingWorker {
    /// Create a new processing worker
    pub fn new(state: AppState, job_queue: Arc<JobQueue>) -> Self {
        let cpu_count = num_cpus::get();
        let config = state.config();

        let parallel_files = config.processing.parallel_files
            .unwrap_or_else(|| cpu_count.min(8));
        let parallel_embeddings = config.processing.parallel_embeddings
            .unwrap_or_else(|| cpu_count.min(4));
        let file_timeout = Duration::from_secs(config.processing.file_timeout_secs);

        tracing::info!(
            "Worker configured: {} parallel files, {} parallel embeddings per file, {}s timeout",
            parallel_files,
            parallel_embeddings,
            config.processing.file_timeout_secs
        );

        Self {
            state,
            job_queue,
            parallel_files,
            parallel_embeddings,
            file_timeout,
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
        let file_timeout = self.file_timeout;

        // Create semaphore to limit concurrent file processing
        let semaphore = Arc::new(Semaphore::new(self.parallel_files));

        // Create futures for all files
        let file_futures: Vec<_> = job.files.into_iter().map(|file_data| {
            let state = self.state.clone();
            let job_queue = self.job_queue.clone();
            let sem = semaphore.clone();
            let filename = file_data.filename.clone();
            let file_size = file_data.data.len();

            async move {
                // Acquire semaphore permit
                let _permit = sem.acquire().await.unwrap();

                tracing::info!("Starting parallel processing: {} ({} bytes)", filename, file_size);
                job_queue.update_current_file(job_id, &filename);
                let start_time = std::time::Instant::now();

                // Process the file with timeout
                let process_future = Self::process_single_file(
                    &state,
                    &job_queue,
                    job_id,
                    file_data,
                    parallel_embeddings,
                );

                let result = match timeout(file_timeout, process_future).await {
                    Ok(inner_result) => inner_result,
                    Err(_) => {
                        let elapsed = start_time.elapsed();
                        tracing::error!(
                            "TIMEOUT processing '{}' after {:.1}s (limit: {}s, size: {} bytes). \
                            Possible causes: large file, slow embedding service, or parsing hang.",
                            filename,
                            elapsed.as_secs_f64(),
                            file_timeout.as_secs(),
                            file_size
                        );
                        Err(Error::Internal(format!(
                            "Processing timeout after {}s - file may be too large or complex (size: {} bytes)",
                            file_timeout.as_secs(),
                            file_size
                        )))
                    }
                };

                let elapsed = start_time.elapsed();
                if elapsed.as_secs() > 60 {
                    tracing::warn!(
                        "Slow processing for '{}': took {:.1}s",
                        filename,
                        elapsed.as_secs_f64()
                    );
                }

                (filename, result)
            }
        }).collect();

        // Wait for all files to complete
        let results = join_all(file_futures).await;

        // Process results
        for (filename, result) in results {
            match result {
                Ok(FileProcessResult::New { document, file_size }) => {
                    // Record success in file registry
                    self.state.record_file_success(
                        &filename,
                        &document.content_hash,
                        file_size,
                        document.file_type.clone(),
                        document.id,
                        document.total_chunks,
                        Some(job_id),
                    );
                    self.state.add_document(document);
                    self.job_queue.increment_files_processed(job_id);
                    tracing::info!("Processed new file: {}", filename);
                }
                Ok(FileProcessResult::Updated { document, file_size, old_chunks_deleted }) => {
                    // Record success in file registry
                    self.state.record_file_success(
                        &filename,
                        &document.content_hash,
                        file_size,
                        document.file_type.clone(),
                        document.id,
                        document.total_chunks,
                        Some(job_id),
                    );
                    self.state.add_document(document);
                    self.job_queue.increment_files_processed(job_id);
                    tracing::info!("Updated file: {}, deleted {} old chunks", filename, old_chunks_deleted);
                }
                Ok(FileProcessResult::Skipped { reason, skip_reason, content_hash, file_size, file_type }) => {
                    // Record skip in file registry
                    self.state.record_file_skipped(
                        &filename,
                        &content_hash,
                        file_size,
                        file_type,
                        skip_reason,
                        Some(job_id),
                    );
                    tracing::info!("Skipped {}: {}", filename, reason);
                    self.job_queue.add_skipped_file(job_id, &filename, &reason);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Record failure in file registry
                    let ext = filename.rsplit('.').next().unwrap_or("");
                    let file_type = FileType::from_extension(ext);
                    // We don't have content hash for failed files, use empty string
                    self.state.record_file_failed(
                        &filename,
                        "",
                        0,
                        file_type,
                        &error_msg,
                        "parsing",
                        Some(job_id),
                    );
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
        let file_size = data.len();

        // Individual operation timeout (2 minutes for conversions/parsing)
        let op_timeout = Duration::from_secs(120);

        tracing::info!("[{}] Starting processing ({} bytes)", filename, file_size);

        // Check file extension
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

        // Store original filename for reporting/citations
        let original_filename = filename.to_string();

        // For PDFs, try pdftotext first (much faster and handles fonts better)
        if ext == "pdf" && ExternalParser::has_pdftotext() {
            tracing::info!("[{}] Using pdftotext for PDF extraction", filename);
            match external_parser.convert_pdf_with_pdftotext(data) {
                Ok(text) => {
                    tracing::info!("[{}] pdftotext extracted {} chars", filename, text.len());
                    // Create a text file from the extracted content
                    let text_filename = format!("{}.txt", filename.trim_end_matches(".pdf"));
                    return Self::process_text_content(
                        state,
                        job_queue,
                        job_id,
                        &original_filename,  // Use original filename for display
                        Some(&text_filename), // Internal filename
                        text.as_bytes(),
                        Some(data),          // Original PDF data for GCS storage
                        parallel_embeddings,
                    ).await;
                }
                Err(e) => {
                    tracing::warn!("[{}] pdftotext failed: {}, falling back to Rust parser", filename, e);
                    // Continue with normal processing
                }
            }
        }

        // Check if we need to convert legacy format
        let (processed_filename, processed_data) = if ExternalParser::needs_conversion(filename) {
            tracing::info!("[{}] Converting legacy format with LibreOffice...", filename);
            let convert_result = timeout(
                op_timeout,
                external_parser.convert_with_libreoffice(filename, data)
            ).await;

            match convert_result {
                Ok(Ok(converted)) => {
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
                    tracing::info!("[{}] LibreOffice conversion successful", filename);
                    (format!("{}.{}", stem, new_ext), converted)
                }
                Ok(Err(e)) => {
                    tracing::warn!("[{}] LibreOffice conversion failed: {}, trying external API", filename, e);
                    let parse_result = timeout(
                        op_timeout,
                        external_parser.parse_with_unstructured(filename, data)
                    ).await;
                    match parse_result {
                        Ok(Ok(parsed)) => (format!("{}.txt", filename), parsed.content.into_bytes()),
                        Ok(Err(e)) => {
                            tracing::error!("[{}] External parser failed: {}", filename, e);
                            return Err(e);
                        }
                        Err(_) => {
                            tracing::error!("[{}] TIMEOUT: External parser took >{}s", filename, op_timeout.as_secs());
                            return Err(Error::Internal(format!(
                                "External parser timeout after {}s for '{}'", op_timeout.as_secs(), filename
                            )));
                        }
                    }
                }
                Err(_) => {
                    tracing::error!("[{}] TIMEOUT: LibreOffice conversion took >{}s", filename, op_timeout.as_secs());
                    return Err(Error::Internal(format!(
                        "LibreOffice conversion timeout after {}s for '{}'", op_timeout.as_secs(), filename
                    )));
                }
            }
        } else if ExternalParser::needs_ocr(filename) {
            // Image files - try OCR
            tracing::info!("[{}] Using OCR for image extraction...", filename);

            // Try local tesseract first
            if ExternalParser::has_tesseract() {
                match external_parser.convert_image_with_ocr(data) {
                    Ok(text) => {
                        tracing::info!("[{}] OCR extracted {} chars", filename, text.len());
                        let text_filename = format!("{}.txt", filename.rsplit('.').next().unwrap_or(filename));
                        return Self::process_text_content(
                            state,
                            job_queue,
                            job_id,
                            &original_filename,
                            Some(&text_filename),
                            text.as_bytes(),
                            Some(data),
                            parallel_embeddings,
                        ).await;
                    }
                    Err(e) => {
                        tracing::warn!("[{}] Local OCR failed: {}, trying Unstructured API", filename, e);
                    }
                }
            }

            // Fall back to Unstructured API (has built-in OCR)
            let parse_result = timeout(
                op_timeout,
                external_parser.parse_with_unstructured(filename, data)
            ).await;
            match parse_result {
                Ok(Ok(parsed)) => {
                    tracing::info!("[{}] Unstructured OCR successful", filename);
                    (format!("{}.txt", filename), parsed.content.into_bytes())
                }
                Ok(Err(e)) => {
                    tracing::error!("[{}] Image OCR failed: {}", filename, e);
                    return Err(Error::file_parse(
                        filename,
                        format!("Image OCR failed. Install tesseract-ocr for local OCR. Error: {}", e)
                    ));
                }
                Err(_) => {
                    return Err(Error::file_parse(
                        filename,
                        "Image OCR timeout".to_string()
                    ));
                }
            }
        } else if ExternalParser::needs_external_parsing(filename) {
            // Other formats needing external parsing (RTF, ODT, ODP, ODS, EPUB)
            tracing::info!("[{}] Using external parser...", filename);

            // Try pandoc first for supported formats
            if ExternalParser::has_pandoc() && ExternalParser::can_use_pandoc(filename) {
                match external_parser.convert_with_pandoc(filename, data) {
                    Ok(text) => {
                        tracing::info!("[{}] pandoc extracted {} chars", filename, text.len());
                        let text_filename = format!("{}.txt", filename.rsplit('.').next().unwrap_or(filename));
                        return Self::process_text_content(
                            state,
                            job_queue,
                            job_id,
                            &original_filename,
                            Some(&text_filename),
                            text.as_bytes(),
                            Some(data),
                            parallel_embeddings,
                        ).await;
                    }
                    Err(e) => {
                        tracing::warn!("[{}] pandoc failed: {}, trying Unstructured API", filename, e);
                    }
                }
            }

            // Fall back to Unstructured API
            let parse_result = timeout(
                op_timeout,
                external_parser.parse_with_unstructured(filename, data)
            ).await;
            match parse_result {
                Ok(Ok(parsed)) => {
                    tracing::info!("[{}] External parsing successful", filename);
                    (format!("{}.txt", filename), parsed.content.into_bytes())
                }
                Ok(Err(e)) => {
                    tracing::error!("[{}] External parser failed: {}", filename, e);
                    return Err(e);
                }
                Err(_) => {
                    tracing::error!("[{}] TIMEOUT: External parser took >{}s", filename, op_timeout.as_secs());
                    return Err(Error::Internal(format!(
                        "External parser timeout after {}s for '{}'", op_timeout.as_secs(), filename
                    )));
                }
            }
        } else {
            (filename.to_string(), data.to_vec())
        };

        tracing::info!("[{}] Parsing file content...", original_filename);

        // Determine internal filename if different from original
        let internal_filename = if processed_filename != original_filename {
            Some(processed_filename.clone())
        } else {
            None
        };

        // Create pipeline
        let pipeline = IngestPipeline::new(
            config.chunking.chunk_size,
            config.chunking.chunk_overlap,
        );

        // Parse file to get content hash, with fallback for PDFs
        let parsed = match pipeline.parse_file(&processed_filename, &processed_data) {
            Ok(p) => p,
            Err(e) if ext == "pdf" => {
                // PDF parsing failed, try comprehensive fallback (OCR, Unstructured API, Document AI)
                tracing::warn!(
                    "[{}] Native PDF parsing failed: {}. Trying fallback methods...",
                    filename, e
                );

                // First try local tools (pdftotext, OCR)
                match timeout(
                    op_timeout,
                    external_parser.convert_pdf_comprehensive(data)
                ).await {
                    Ok(Ok((text, method))) => {
                        tracing::info!(
                            "[{}] Fallback succeeded via {}, extracted {} chars",
                            filename, method, text.len()
                        );
                        // Process as text content
                        let text_filename = format!("{}.txt", filename.trim_end_matches(".pdf"));
                        return Self::process_text_content(
                            state,
                            job_queue,
                            job_id,
                            &original_filename,
                            Some(&text_filename),
                            text.as_bytes(),
                            Some(data),
                            parallel_embeddings,
                        ).await;
                    }
                    Ok(Err(fallback_err)) => {
                        // Local methods failed, try Document AI if available (GCP only)
                        #[cfg(feature = "gcp")]
                        if let Some(doc_ai) = state.document_ai() {
                            tracing::info!(
                                "[{}] Local PDF extraction failed, trying Document AI...",
                                filename
                            );
                            match timeout(
                                Duration::from_secs(180), // 3 min for Document AI
                                doc_ai.process_pdf(data, filename)
                            ).await {
                                Ok(Ok(result)) => {
                                    tracing::info!(
                                        "[{}] Document AI succeeded, extracted {} chars from {} pages",
                                        filename, result.text.len(), result.total_pages
                                    );
                                    let text_filename = format!("{}.txt", filename.trim_end_matches(".pdf"));
                                    return Self::process_text_content(
                                        state,
                                        job_queue,
                                        job_id,
                                        &original_filename,
                                        Some(&text_filename),
                                        result.text.as_bytes(),
                                        Some(data),
                                        parallel_embeddings,
                                    ).await;
                                }
                                Ok(Err(doc_ai_err)) => {
                                    tracing::error!(
                                        "[{}] Document AI also failed: {}",
                                        filename, doc_ai_err
                                    );
                                }
                                Err(_) => {
                                    tracing::error!(
                                        "[{}] Document AI timeout after 180s",
                                        filename
                                    );
                                }
                            }
                        }

                        tracing::error!(
                            "[{}] All PDF extraction methods failed. Original: {}, Fallback: {}",
                            filename, e, fallback_err
                        );
                        return Err(Error::file_parse(
                            filename,
                            format!("PDF extraction failed. Install poppler-utils and tesseract-ocr for better support. Error: {}", e)
                        ));
                    }
                    Err(_) => {
                        tracing::error!("[{}] PDF fallback timeout after {}s", filename, op_timeout.as_secs());
                        return Err(Error::file_parse(
                            filename,
                            format!("PDF extraction timeout. The file may be too large or complex.")
                        ));
                    }
                }
            }
            Err(e) => return Err(e),
        };

        // Check file status for deduplication (use original filename for tracking)
        match state.check_file_status(&original_filename, &parsed.content_hash) {
            FileStatus::Unchanged(existing) => {
                return Ok(FileProcessResult::Skipped {
                    reason: format!(
                        "unchanged (hash: {}...)",
                        &existing.content_hash[..12.min(existing.content_hash.len())]
                    ),
                    skip_reason: SkipReason::Unchanged,
                    content_hash: existing.content_hash.clone(),
                    file_size: file_size as u64,
                    file_type: parsed.file_type.clone(),
                });
            }
            FileStatus::Duplicate(existing) => {
                return Ok(FileProcessResult::Skipped {
                    reason: format!("duplicate of '{}'", existing.filename),
                    skip_reason: SkipReason::Duplicate { existing_filename: existing.filename.clone() },
                    content_hash: existing.content_hash.clone(),
                    file_size: file_size as u64,
                    file_type: parsed.file_type.clone(),
                });
            }
            FileStatus::ExistsInRegistry(record) => {
                return Ok(FileProcessResult::Skipped {
                    reason: format!(
                        "already in GCS (hash: {}..., uploaded: {})",
                        &record.content_hash[..12.min(record.content_hash.len())],
                        record.first_seen_at.format("%Y-%m-%d")
                    ),
                    skip_reason: SkipReason::Unchanged,
                    content_hash: record.content_hash.clone(),
                    file_size: file_size as u64,
                    file_type: parsed.file_type.clone(),
                });
            }
            FileStatus::DuplicateInRegistry(record) => {
                return Ok(FileProcessResult::Skipped {
                    reason: format!("duplicate of '{}' in GCS", record.filename),
                    skip_reason: SkipReason::Duplicate { existing_filename: record.filename.clone() },
                    content_hash: record.content_hash.clone(),
                    file_size: file_size as u64,
                    file_type: parsed.file_type.clone(),
                });
            }
            FileStatus::Modified(existing) => {
                // Delete old document and its chunks
                let deleted = state.delete_document_with_chunks(&existing.id)?;
                tracing::info!(
                    "File '{}' modified, deleted {} old chunks",
                    original_filename,
                    deleted
                );

                // Process the new version
                let doc = Self::process_file_content(
                    state,
                    job_queue,
                    job_id,
                    &original_filename,
                    internal_filename.as_deref(),
                    &processed_data,
                    &parsed,
                    parallel_embeddings,
                ).await?;
                return Ok(FileProcessResult::Updated {
                    document: doc,
                    file_size: file_size as u64,
                    old_chunks_deleted: deleted,
                });
            }
            FileStatus::New => {
                // Process new file
                let doc = Self::process_file_content(
                    state,
                    job_queue,
                    job_id,
                    &original_filename,
                    internal_filename.as_deref(),
                    &processed_data,
                    &parsed,
                    parallel_embeddings,
                ).await?;
                return Ok(FileProcessResult::New {
                    document: doc,
                    file_size: file_size as u64,
                });
            }
        }
    }

    /// Process pre-extracted text content (for pdftotext/pandoc output)
    /// - `original_filename`: The filename as uploaded by user (used for display/citations)
    /// - `internal_filename`: The converted filename if different (e.g., "report.pdf" -> "report.txt")
    /// - `original_data`: Original file bytes for GCS storage (optional)
    /// Returns FileProcessResult to properly handle skipped files
    async fn process_text_content(
        state: &AppState,
        job_queue: &Arc<JobQueue>,
        job_id: uuid::Uuid,
        original_filename: &str,
        internal_filename: Option<&str>,
        text_data: &[u8],
        original_data: Option<&[u8]>,
        parallel_embeddings: usize,
    ) -> Result<FileProcessResult> {
        let config = state.config();
        let content = String::from_utf8_lossy(text_data).to_string();

        // Hash the content
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        let text_size = text_data.len() as u64;
        let original_size = original_data.map(|d| d.len() as u64).unwrap_or(text_size);

        // Check for duplicates using original filename - return Skipped, not Error
        match state.check_file_status(original_filename, &content_hash) {
            crate::server::state::FileStatus::Unchanged(existing) => {
                tracing::info!("[{}] Unchanged, skipping", original_filename);
                return Ok(FileProcessResult::Skipped {
                    reason: format!(
                        "unchanged (hash: {}...)",
                        &existing.content_hash[..12.min(existing.content_hash.len())]
                    ),
                    skip_reason: SkipReason::Unchanged,
                    content_hash: existing.content_hash.clone(),
                    file_size: original_size,
                    file_type: crate::types::FileType::Txt,
                });
            }
            crate::server::state::FileStatus::Duplicate(existing) => {
                tracing::info!("[{}] Duplicate of {}, skipping", original_filename, existing.filename);
                return Ok(FileProcessResult::Skipped {
                    reason: format!("duplicate of '{}'", existing.filename),
                    skip_reason: SkipReason::Duplicate { existing_filename: existing.filename.clone() },
                    content_hash: existing.content_hash.clone(),
                    file_size: original_size,
                    file_type: crate::types::FileType::Txt,
                });
            }
            crate::server::state::FileStatus::ExistsInRegistry(record) => {
                tracing::info!("[{}] Already in GCS registry, skipping", original_filename);
                return Ok(FileProcessResult::Skipped {
                    reason: format!(
                        "already in GCS (hash: {}..., uploaded: {})",
                        &record.content_hash[..12.min(record.content_hash.len())],
                        record.first_seen_at.format("%Y-%m-%d")
                    ),
                    skip_reason: SkipReason::Unchanged,
                    content_hash: record.content_hash.clone(),
                    file_size: original_size,
                    file_type: crate::types::FileType::Txt,
                });
            }
            crate::server::state::FileStatus::DuplicateInRegistry(record) => {
                tracing::info!("[{}] Duplicate of {} in GCS, skipping", original_filename, record.filename);
                return Ok(FileProcessResult::Skipped {
                    reason: format!("duplicate of '{}' in GCS", record.filename),
                    skip_reason: SkipReason::Duplicate { existing_filename: record.filename.clone() },
                    content_hash: record.content_hash.clone(),
                    file_size: original_size,
                    file_type: crate::types::FileType::Txt,
                });
            }
            crate::server::state::FileStatus::Modified(existing) => {
                // Delete old document and its chunks, then continue processing
                let deleted = state.delete_document_with_chunks(&existing.id)?;
                tracing::info!(
                    "[{}] File modified, deleted {} old chunks, reprocessing",
                    original_filename,
                    deleted
                );
                // Continue to process the new version below
            }
            crate::server::state::FileStatus::New => {
                // Continue to process new file below
            }
        }

        // Create document with original and internal filenames
        let mut doc = if let Some(internal) = internal_filename {
            Document::new_with_internal(
                original_filename.to_string(),
                internal.to_string(),
                crate::types::FileType::Txt,
                content_hash,
                text_data.len() as u64,
            )
        } else {
            Document::new(
                original_filename.to_string(),
                crate::types::FileType::Txt,
                content_hash,
                text_data.len() as u64,
            )
        };

        // Create pipeline for chunking
        let pipeline = IngestPipeline::new(
            config.chunking.chunk_size,
            config.chunking.chunk_overlap,
        );

        // Create a parsed document structure
        let parsed = crate::ingestion::ParsedDocument {
            file_type: crate::types::FileType::Txt,
            content: content.clone(),
            content_hash: doc.content_hash.clone(),
            total_pages: Some(1),
            pages: vec![crate::ingestion::PageContent {
                page_number: 1,
                content: content.clone(),
                char_offset: 0,
            }],
            metadata: std::collections::HashMap::new(),
        };

        // Create chunks
        tracing::info!("[{}] Creating chunks from extracted text...", original_filename);
        let mut chunks = pipeline.create_chunks(&doc, &parsed)?;
        let total_chunks = chunks.len();
        tracing::info!("[{}] Created {} chunks, generating embeddings...", original_filename, total_chunks);

        // Generate embeddings using provider abstraction
        let chunk_batches: Vec<_> = chunks.chunks_mut(parallel_embeddings).collect();
        let embedding_provider = state.embedding_provider();
        let embed_timeout = Duration::from_secs(60);
        let mut batch_num = 0;
        let total_batches = chunk_batches.len();

        for batch in chunk_batches {
            batch_num += 1;
            let batch_start = std::time::Instant::now();

            let embedding_futures: Vec<_> = batch
                .iter()
                .map(|chunk| embedding_provider.embed(&chunk.content))
                .collect();

            let batch_result = timeout(embed_timeout, join_all(embedding_futures)).await;

            match batch_result {
                Ok(results) => {
                    for (chunk, result) in batch.iter_mut().zip(results) {
                        match result {
                            Ok(embedding) => chunk.embedding = embedding,
                            Err(e) => {
                                tracing::warn!("[{}] Embedding failed: {}", original_filename, e);
                                chunk.embedding = vec![0.0; config.embeddings.dimensions];
                            }
                        }
                    }
                }
                Err(_) => {
                    tracing::error!("[{}] Embedding batch {}/{} timed out", original_filename, batch_num, total_batches);
                    for chunk in batch.iter_mut() {
                        chunk.embedding = vec![0.0; config.embeddings.dimensions];
                    }
                }
            }

            if batch_start.elapsed().as_secs() > 10 {
                tracing::info!("[{}] Batch {}/{} took {:.1}s", original_filename, batch_num, total_batches, batch_start.elapsed().as_secs_f64());
            }

            job_queue.increment_chunks_embedded(job_id, batch.len());
        }

        // Store chunks using provider (Vertex AI for GCP backend)
        tracing::info!("[{}] Storing {} chunks...", original_filename, total_chunks);
        state.vector_store_provider().insert_chunks(&chunks).await?;

        // Store chunks locally for metadata lookup (needed for Vertex AI)
        state.store_chunks(&chunks);

        // Store original file and plain text in GCS (GCP backend only)
        #[cfg(feature = "gcp")]
        if let Some(document_store) = state.document_store() {
            // Store original file if provided
            if let Some(orig_data) = original_data {
                match document_store.store_document(&doc.id, original_filename, orig_data).await {
                    Ok(original_uri) => {
                        doc.metadata.insert("original_uri".to_string(), serde_json::Value::String(original_uri));
                        tracing::debug!("[{}] Stored original in GCS", original_filename);
                    }
                    Err(e) => {
                        tracing::warn!("[{}] Failed to store original in GCS: {}", original_filename, e);
                    }
                }
            }

            // Store plain text
            match document_store.store_plain_text(&doc.id, original_filename, &content).await {
                Ok(plaintext_uri) => {
                    doc.metadata.insert("plaintext_uri".to_string(), serde_json::Value::String(plaintext_uri));
                    tracing::debug!("[{}] Stored plain text in GCS", original_filename);
                }
                Err(e) => {
                    tracing::warn!("[{}] Failed to store plain text in GCS: {}", original_filename, e);
                }
            }
        }

        doc.total_chunks = total_chunks as u32;
        tracing::info!("[{}] COMPLETE: {} chunks stored", original_filename, total_chunks);

        Ok(FileProcessResult::New {
            document: doc,
            file_size: original_size,
        })
    }

    /// Process file content (chunking, embedding, storing)
    /// - `original_filename`: The filename as uploaded by user (used for display/citations)
    /// - `internal_filename`: The converted filename if different (e.g., "doc.doc" -> "doc.docx")
    async fn process_file_content(
        state: &AppState,
        job_queue: &Arc<JobQueue>,
        job_id: uuid::Uuid,
        original_filename: &str,
        internal_filename: Option<&str>,
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

        // Create document with original and internal filenames
        let mut doc = if let Some(internal) = internal_filename {
            Document::new_with_internal(
                original_filename.to_string(),
                internal.to_string(),
                parsed.file_type.clone(),
                parsed.content_hash.clone(),
                data.len() as u64,
            )
        } else {
            Document::new(
                original_filename.to_string(),
                parsed.file_type.clone(),
                parsed.content_hash.clone(),
                data.len() as u64,
            )
        };
        doc.total_pages = parsed.total_pages;

        // Create chunks
        tracing::info!("[{}] Creating chunks...", original_filename);
        let mut chunks = pipeline.create_chunks(&doc, parsed)?;
        let total_chunks = chunks.len();
        tracing::info!("[{}] Created {} chunks, generating embeddings...", original_filename, total_chunks);

        // Generate embeddings in parallel batches with timeout (using provider abstraction)
        let chunk_batches: Vec<_> = chunks.chunks_mut(parallel_embeddings).collect();
        let embedding_provider = state.embedding_provider();
        let embed_timeout = Duration::from_secs(60); // 60s per batch
        let mut batch_num = 0;
        let total_batches = chunk_batches.len();

        for batch in chunk_batches {
            batch_num += 1;
            let batch_start = std::time::Instant::now();

            let embedding_futures: Vec<_> = batch
                .iter()
                .map(|chunk| embedding_provider.embed(&chunk.content))
                .collect();

            // Wrap the batch in a timeout
            let batch_result = timeout(embed_timeout, join_all(embedding_futures)).await;

            match batch_result {
                Ok(results) => {
                    let mut failed_count = 0;
                    for (chunk, result) in batch.iter_mut().zip(results) {
                        match result {
                            Ok(embedding) => {
                                chunk.embedding = embedding;
                            }
                            Err(e) => {
                                failed_count += 1;
                                tracing::warn!("[{}] Embedding failed for chunk: {}", original_filename, e);
                                // Use zero vector as fallback
                                chunk.embedding = vec![0.0; config.embeddings.dimensions];
                            }
                        }
                    }
                    if failed_count > 0 {
                        tracing::warn!(
                            "[{}] Batch {}/{}: {} embeddings failed, using fallback",
                            original_filename, batch_num, total_batches, failed_count
                        );
                    }
                }
                Err(_) => {
                    tracing::error!(
                        "[{}] TIMEOUT: Embedding batch {}/{} took >{}s, using fallback embeddings",
                        original_filename, batch_num, total_batches, embed_timeout.as_secs()
                    );
                    // Use zero vectors for all chunks in this batch
                    for chunk in batch.iter_mut() {
                        chunk.embedding = vec![0.0; config.embeddings.dimensions];
                    }
                }
            }

            let batch_elapsed = batch_start.elapsed();
            if batch_elapsed.as_secs() > 10 {
                tracing::info!(
                    "[{}] Batch {}/{} took {:.1}s",
                    original_filename, batch_num, total_batches, batch_elapsed.as_secs_f64()
                );
            }

            job_queue.increment_chunks_embedded(job_id, batch.len());
        }

        // Store chunks using provider (Vertex AI for GCP backend)
        tracing::info!("[{}] Storing {} chunks in vector database...", original_filename, total_chunks);
        state.vector_store_provider().insert_chunks(&chunks).await?;

        // Store chunks locally for metadata lookup (needed for Vertex AI)
        state.store_chunks(&chunks);

        // Store original file and plain text in GCS (GCP backend only)
        #[cfg(feature = "gcp")]
        if let Some(document_store) = state.document_store() {
            // Store original file
            match document_store.store_document(&doc.id, original_filename, data).await {
                Ok(original_uri) => {
                    doc.metadata.insert("original_uri".to_string(), serde_json::Value::String(original_uri));
                    tracing::debug!("[{}] Stored original in GCS", original_filename);
                }
                Err(e) => {
                    tracing::warn!("[{}] Failed to store original in GCS: {}", original_filename, e);
                }
            }

            // Store extracted plain text
            match document_store.store_plain_text(&doc.id, original_filename, &parsed.content).await {
                Ok(plaintext_uri) => {
                    doc.metadata.insert("plaintext_uri".to_string(), serde_json::Value::String(plaintext_uri));
                    tracing::debug!("[{}] Stored plain text in GCS", original_filename);
                }
                Err(e) => {
                    tracing::warn!("[{}] Failed to store plain text in GCS: {}", original_filename, e);
                }
            }
        }

        doc.total_chunks = total_chunks as u32;

        tracing::info!(
            "[{}] COMPLETE: {} pages, {} chunks stored",
            original_filename,
            doc.total_pages.unwrap_or(1),
            total_chunks
        );

        Ok(doc)
    }
}
