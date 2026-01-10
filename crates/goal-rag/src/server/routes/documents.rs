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
    /// Organization ID for multi-tenancy (REQUIRED for tenant isolation)
    pub organization_id: String,
}

/// GET /api/documents - List all documents for an organization
pub async fn list_documents(
    State(state): State<AppState>,
    Query(query): Query<ListDocumentsQuery>,
) -> Result<Json<DocumentListResponse>> {
    let all_documents = state.list_documents();

    // Filter by organization_id (required for multi-tenancy)
    let filtered_documents: Vec<_> = all_documents
        .into_iter()
        .filter(|doc| doc.organization_id.as_ref() == Some(&query.organization_id))
        .collect();

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

/// Query parameters for document operations requiring org context
#[derive(Debug, Deserialize)]
pub struct OrgQuery {
    /// Organization ID for multi-tenancy (REQUIRED for tenant isolation)
    pub organization_id: String,
}

/// GET /api/documents/:id - Get a specific document
pub async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<OrgQuery>,
) -> Result<Json<DocumentSummary>> {
    let doc = state
        .get_document(&id)
        .ok_or_else(|| Error::DocumentNotFound(id.to_string()))?;

    // Verify document belongs to the requested organization
    if doc.organization_id.as_ref() != Some(&query.organization_id) {
        return Err(Error::DocumentNotFound(format!(
            "Document {} not found in organization {}",
            id, query.organization_id
        )));
    }

    Ok(Json(DocumentSummary::from(&doc)))
}

/// DELETE /api/documents/:id - Delete a document
pub async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<OrgQuery>,
) -> Result<Json<serde_json::Value>> {
    // First verify the document belongs to the organization
    let doc = state
        .get_document(&id)
        .ok_or_else(|| Error::DocumentNotFound(id.to_string()))?;

    // Verify document belongs to the requested organization
    if doc.organization_id.as_ref() != Some(&query.organization_id) {
        return Err(Error::DocumentNotFound(format!(
            "Document {} not found in organization {}",
            id, query.organization_id
        )));
    }

    // Remove document from registry
    let doc = state
        .remove_document(&id)
        .ok_or_else(|| Error::DocumentNotFound(id.to_string()))?;

    // Delete all chunks for this document (uses provider abstraction)
    let deleted_chunks = state.vector_store_provider().delete_by_document(&id).await?;

    tracing::info!(
        "Deleted document '{}' from org '{}' and {} chunks",
        doc.filename,
        query.organization_id,
        deleted_chunks
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "document_id": id,
        "organization_id": query.organization_id,
        "filename": doc.filename,
        "deleted_chunks": deleted_chunks
    })))
}
