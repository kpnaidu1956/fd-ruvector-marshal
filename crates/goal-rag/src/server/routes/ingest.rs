//! Document ingestion endpoint

use axum::{
    extract::{Multipart, State},
    Json,
};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::ingestion::{ExternalParser, IngestPipeline};
#[cfg(feature = "gcp")]
use crate::providers::document_store::DocumentStoreProvider;
use crate::server::state::{AppState, FileStatus};
use crate::types::{
    query::IngestOptions,
    response::{DocumentSummary, IngestError, IngestResponse},
    Document,
};

/// POST /api/ingest - Upload and process files
pub async fn ingest_files(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<IngestResponse>> {
    let start = Instant::now();
    let mut documents = Vec::new();
    let mut errors = Vec::new();
    let mut total_chunks = 0u32;

    // Parse options from first field if it's JSON
    let mut options = IngestOptions::default();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        Error::Internal(format!("Failed to read multipart field: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();

        // Check if this is the options field
        if name == "options" {
            let data = field.bytes().await.map_err(|e| {
                Error::Internal(format!("Failed to read options: {}", e))
            })?;
            if let Ok(opts) = serde_json::from_slice::<IngestOptions>(&data) {
                options = opts;
            }
            continue;
        }

        // Get filename
        let filename = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("file_{}.bin", Uuid::new_v4()));

        // Read file content
        let data = match field.bytes().await {
            Ok(d) => d,
            Err(e) => {
                errors.push(IngestError {
                    filename: filename.clone(),
                    error: format!("Failed to read file: {}", e),
                });
                continue;
            }
        };

        tracing::info!("Processing file: {} ({} bytes)", filename, data.len());

        // Check if file needs conversion (legacy formats)
        let (processed_filename, processed_data) = if ExternalParser::needs_conversion(&filename) {
            tracing::info!("Converting legacy format: {}", filename);
            match convert_legacy_format(&state, &filename, &data).await {
                Ok((new_name, new_data)) => (new_name, new_data),
                Err(e) => {
                    tracing::warn!("Conversion failed, trying external parsing: {}", e);
                    // Fall back to external parsing
                    match parse_with_external(&state, &filename, &data).await {
                        Ok(content) => {
                            let new_name = format!("{}.txt", filename);
                            (new_name, content.into_bytes())
                        }
                        Err(e2) => {
                            errors.push(IngestError {
                                filename,
                                error: format!("Failed to process legacy format: {} / {}", e, e2),
                            });
                            continue;
                        }
                    }
                }
            }
        } else if ExternalParser::needs_external_parsing(&filename) {
            // Use external API for other unsupported formats
            match parse_with_external(&state, &filename, &data).await {
                Ok(content) => {
                    let new_name = format!("{}.txt", filename);
                    (new_name, content.into_bytes())
                }
                Err(e) => {
                    errors.push(IngestError {
                        filename,
                        error: format!("External parsing failed: {}", e),
                    });
                    continue;
                }
            }
        } else {
            (filename.clone(), data.to_vec())
        };

        // Process the file with deduplication and timeout
        let file_timeout = Duration::from_secs(state.config().processing.file_timeout_secs);
        let file_size = processed_data.len();
        let file_start = Instant::now();

        let process_result = timeout(
            file_timeout,
            process_file_with_dedup(&state, &processed_filename, &processed_data, &options)
        ).await;

        match process_result {
            Ok(Ok(ProcessResult::New(doc, chunk_count))) => {
                total_chunks += chunk_count;
                documents.push(DocumentSummary::from(&doc));
                state.add_document(doc);
                tracing::info!("Ingested new file: {} in {:.1}s", processed_filename, file_start.elapsed().as_secs_f64());
            }
            Ok(Ok(ProcessResult::Updated(doc, chunk_count, old_chunks_deleted))) => {
                total_chunks += chunk_count;
                documents.push(DocumentSummary::from(&doc));
                state.add_document(doc);
                tracing::info!(
                    "Updated file: {} (deleted {} old chunks, created {} new) in {:.1}s",
                    processed_filename,
                    old_chunks_deleted,
                    chunk_count,
                    file_start.elapsed().as_secs_f64()
                );
            }
            Ok(Ok(ProcessResult::Skipped(reason))) => {
                tracing::info!("Skipped file: {} ({})", filename, reason);
                // Not an error, but we don't add to documents list
            }
            Ok(Err(e)) => {
                tracing::error!("Failed to process {}: {}", filename, e);
                errors.push(IngestError {
                    filename,
                    error: e.to_string(),
                });
            }
            Err(_) => {
                // Timeout occurred
                let elapsed = file_start.elapsed();
                tracing::error!(
                    "TIMEOUT processing '{}' after {:.1}s (limit: {}s, size: {} bytes). \
                    Skipping file. Possible causes: large file, slow embedding service, or parsing hang.",
                    filename,
                    elapsed.as_secs_f64(),
                    file_timeout.as_secs(),
                    file_size
                );
                errors.push(IngestError {
                    filename,
                    error: format!(
                        "Processing timeout after {}s - file may be too large or complex (size: {} bytes)",
                        file_timeout.as_secs(),
                        file_size
                    ),
                });
            }
        }
    }

    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok(Json(IngestResponse {
        success: !documents.is_empty(),
        documents,
        total_chunks_created: total_chunks,
        processing_time_ms,
        errors,
    }))
}

/// Result of processing a file with deduplication
enum ProcessResult {
    /// New file, successfully processed
    New(Document, u32),
    /// File was modified, old chunks deleted and new ones created
    Updated(Document, u32, usize),
    /// File was skipped (unchanged or duplicate)
    Skipped(String),
}

/// Process a single file with deduplication check
async fn process_file_with_dedup(
    state: &AppState,
    filename: &str,
    data: &[u8],
    options: &IngestOptions,
) -> Result<ProcessResult> {
    let config = state.config();

    // Create ingestion pipeline
    let pipeline = IngestPipeline::new(
        options.chunk_size.unwrap_or(config.chunking.chunk_size),
        options.chunk_overlap.unwrap_or(config.chunking.chunk_overlap),
    );

    // Parse the file to get content hash
    let parsed = pipeline.parse_file(filename, data)?;

    // Check file status for deduplication
    match state.check_file_status(filename, &parsed.content_hash) {
        FileStatus::Unchanged(existing) => {
            return Ok(ProcessResult::Skipped(format!(
                "unchanged (hash: {}...)",
                &existing.content_hash[..12]
            )));
        }
        FileStatus::Duplicate(existing) => {
            return Ok(ProcessResult::Skipped(format!(
                "duplicate of '{}'",
                existing.filename
            )));
        }
        FileStatus::ExistsInRegistry(record) => {
            return Ok(ProcessResult::Skipped(format!(
                "already in GCS (hash: {}..., uploaded: {})",
                &record.content_hash[..record.content_hash.len().min(12)],
                record.first_seen_at.format("%Y-%m-%d")
            )));
        }
        FileStatus::DuplicateInRegistry(record) => {
            return Ok(ProcessResult::Skipped(format!(
                "duplicate of '{}' in GCS",
                record.filename
            )));
        }
        FileStatus::Modified(existing) => {
            // Delete old document and its chunks
            let deleted = state.delete_document_with_chunks(&existing.id)?;
            tracing::info!(
                "File '{}' modified, deleted {} old chunks",
                filename,
                deleted
            );

            // Process the new version
            let (doc, chunk_count) = process_file_internal(state, filename, data, &parsed, options).await?;
            return Ok(ProcessResult::Updated(doc, chunk_count, deleted));
        }
        FileStatus::New => {
            // Process new file
            let (doc, chunk_count) = process_file_internal(state, filename, data, &parsed, options).await?;
            return Ok(ProcessResult::New(doc, chunk_count));
        }
    }
}

/// Internal file processing (after dedup check)
async fn process_file_internal(
    state: &AppState,
    filename: &str,
    data: &[u8],
    parsed: &crate::ingestion::ParsedDocument,
    options: &IngestOptions,
) -> Result<(Document, u32)> {
    let config = state.config();

    // Create ingestion pipeline
    let pipeline = IngestPipeline::new(
        options.chunk_size.unwrap_or(config.chunking.chunk_size),
        options.chunk_overlap.unwrap_or(config.chunking.chunk_overlap),
    );

    // Create document record
    let mut doc = Document::new(
        filename.to_string(),
        parsed.file_type.clone(),
        parsed.content_hash.clone(),
        data.len() as u64,
    );
    doc.total_pages = parsed.total_pages;
    doc.metadata = options.metadata.clone();

    // Store original file and plain text in GCS (GCP backend only)
    #[cfg(feature = "gcp")]
    if let Some(document_store) = state.document_store() {
        // Store original file
        match document_store.store_document(&doc.id, filename, data).await {
            Ok(original_uri) => {
                doc.metadata.insert("original_uri".to_string(), serde_json::Value::String(original_uri));
                tracing::debug!("Stored original file in GCS: {}", filename);
            }
            Err(e) => {
                tracing::warn!("Failed to store original file in GCS: {}", e);
                // Continue processing - GCS storage is not critical
            }
        }

        // Store extracted plain text
        match document_store.store_plain_text(&doc.id, filename, &parsed.content).await {
            Ok(plaintext_uri) => {
                doc.metadata.insert("plaintext_uri".to_string(), serde_json::Value::String(plaintext_uri));
                tracing::debug!("Stored plain text in GCS: {}", filename);
            }
            Err(e) => {
                tracing::warn!("Failed to store plain text in GCS: {}", e);
                // Continue processing - GCS storage is not critical
            }
        }
    }

    // Create chunks
    let mut chunks = pipeline.create_chunks(&doc, parsed)?;

    // Generate embeddings for all chunks using the embedding provider
    for chunk in chunks.iter_mut() {
        let embedding = state.embedding_provider().embed(&chunk.content).await?;
        chunk.embedding = embedding;
    }

    // Store chunks in vector database (uses Vertex AI for GCP backend)
    let chunk_count = chunks.len() as u32;
    state.vector_store_provider().insert_chunks(&chunks).await?;

    doc.total_chunks = chunk_count;

    tracing::info!(
        "Processed '{}': {} pages, {} chunks",
        filename,
        doc.total_pages.unwrap_or(1),
        chunk_count
    );

    Ok((doc, chunk_count))
}

/// Convert legacy format (DOC, PPT, XLS) using LibreOffice
async fn convert_legacy_format(
    state: &AppState,
    filename: &str,
    data: &[u8],
) -> Result<(String, Vec<u8>)> {
    let converted = state
        .external_parser()
        .convert_with_libreoffice(filename, data)
        .await?;

    // Generate new filename with modern extension
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

    let new_filename = format!("{}.{}", stem, new_ext);
    Ok((new_filename, converted))
}

/// Parse document using external API (Unstructured.io)
async fn parse_with_external(
    state: &AppState,
    filename: &str,
    data: &[u8],
) -> Result<String> {
    let parsed = state
        .external_parser()
        .parse_with_unstructured(filename, data)
        .await?;

    Ok(parsed.content)
}
