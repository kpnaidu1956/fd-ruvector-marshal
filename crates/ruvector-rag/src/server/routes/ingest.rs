//! Document ingestion endpoint

use axum::{
    extract::{Multipart, State},
    Json,
};
use std::time::Instant;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::ingestion::{ExternalParser, IngestPipeline};
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

        // Process the file with deduplication
        match process_file_with_dedup(&state, &processed_filename, &processed_data, &options).await {
            Ok(ProcessResult::New(doc, chunk_count)) => {
                total_chunks += chunk_count;
                documents.push(DocumentSummary::from(&doc));
                state.add_document(doc);
                tracing::info!("Ingested new file: {}", processed_filename);
            }
            Ok(ProcessResult::Updated(doc, chunk_count, old_chunks_deleted)) => {
                total_chunks += chunk_count;
                documents.push(DocumentSummary::from(&doc));
                state.add_document(doc);
                tracing::info!(
                    "Updated file: {} (deleted {} old chunks, created {} new)",
                    processed_filename,
                    old_chunks_deleted,
                    chunk_count
                );
            }
            Ok(ProcessResult::Skipped(reason)) => {
                tracing::info!("Skipped file: {} ({})", filename, reason);
                // Not an error, but we don't add to documents list
            }
            Err(e) => {
                tracing::error!("Failed to process {}: {}", filename, e);
                errors.push(IngestError {
                    filename,
                    error: e.to_string(),
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

    // Create chunks
    let mut chunks = pipeline.create_chunks(&doc, parsed)?;

    // Generate embeddings for all chunks using Ollama
    for chunk in chunks.iter_mut() {
        let embedding = state.ollama().embed(&chunk.content).await?;
        chunk.embedding = embedding;
    }

    // Store chunks in vector database
    let chunk_count = chunks.len() as u32;
    for chunk in &chunks {
        state.vector_store().insert_chunk(chunk)?;
    }

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
