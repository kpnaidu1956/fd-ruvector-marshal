//! Document management endpoints

use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::server::state::AppState;
use crate::types::response::{DocumentListResponse, DocumentSummary};

/// GET /api/documents - List all documents
pub async fn list_documents(
    State(state): State<AppState>,
) -> Result<Json<DocumentListResponse>> {
    let documents: Vec<DocumentSummary> = state
        .list_documents()
        .iter()
        .map(DocumentSummary::from)
        .collect();

    let total_count = documents.len();

    Ok(Json(DocumentListResponse {
        documents,
        total_count,
    }))
}

/// GET /api/documents/:id - Get a specific document
pub async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentSummary>> {
    let doc = state
        .get_document(&id)
        .ok_or_else(|| Error::DocumentNotFound(id.to_string()))?;

    Ok(Json(DocumentSummary::from(&doc)))
}

/// DELETE /api/documents/:id - Delete a document
pub async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    // Remove document from registry
    let doc = state
        .remove_document(&id)
        .ok_or_else(|| Error::DocumentNotFound(id.to_string()))?;

    // Delete all chunks for this document
    let deleted_chunks = state.vector_store().delete_by_document(&id)?;

    tracing::info!(
        "Deleted document '{}' and {} chunks",
        doc.filename,
        deleted_chunks
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "document_id": id,
        "filename": doc.filename,
        "deleted_chunks": deleted_chunks
    })))
}
