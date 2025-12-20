//! Document ingestion endpoint

use axum::{
    extract::{Multipart, State},
    Json,
};
use std::time::Instant;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::ingestion::IngestPipeline;
use crate::server::state::AppState;
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

        // Process the file
        match process_file(&state, &filename, &data, &options).await {
            Ok((doc, chunk_count)) => {
                total_chunks += chunk_count;
                documents.push(DocumentSummary::from(&doc));
                state.add_document(doc);
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

/// Process a single file
async fn process_file(
    state: &AppState,
    filename: &str,
    data: &[u8],
    options: &IngestOptions,
) -> Result<(Document, u32)> {
    let config = state.config();

    // Create ingestion pipeline
    let pipeline = IngestPipeline::new(
        options.chunk_size.unwrap_or(config.chunking.chunk_size),
        options.chunk_overlap.unwrap_or(config.chunking.chunk_overlap),
    );

    // Parse the file
    let parsed = pipeline.parse_file(filename, data)?;

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
    let mut chunks = pipeline.create_chunks(&doc, &parsed)?;

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
        "Ingested '{}': {} pages, {} chunks",
        filename,
        doc.total_pages.unwrap_or(1),
        chunk_count
    );

    Ok((doc, chunk_count))
}
