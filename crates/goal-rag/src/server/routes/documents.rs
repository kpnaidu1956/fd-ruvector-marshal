//! Document management endpoints

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::server::state::AppState;
use crate::types::response::{DocumentListResponse, DocumentSummary};

/// Query parameters for listing documents
#[derive(Debug, Deserialize)]
pub struct ListDocumentsQuery {
    /// Filter by organization ID (multi-tenancy)
    pub organization_id: Option<String>,
}

/// GET /api/documents - List all documents with optional organization filter
pub async fn list_documents(
    State(state): State<AppState>,
    Query(query): Query<ListDocumentsQuery>,
) -> Result<Json<DocumentListResponse>> {
    let all_documents = state.list_documents();

    // Filter by organization_id if provided
    let filtered_documents: Vec<_> = if let Some(ref org_id) = query.organization_id {
        all_documents
            .into_iter()
            .filter(|doc| doc.organization_id.as_ref() == Some(org_id))
            .collect()
    } else {
        all_documents
    };

    let documents: Vec<DocumentSummary> = filtered_documents
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

    // Delete all chunks for this document (uses provider abstraction)
    let deleted_chunks = state.vector_store_provider().delete_by_document(&id).await?;

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
