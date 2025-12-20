//! API routes for the RAG server

pub mod documents;
pub mod ingest;
pub mod query;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};
use crate::server::state::AppState;

/// Build all API routes
pub fn api_routes(max_upload_size: usize) -> Router<AppState> {
    Router::new()
        // Document management
        .route("/documents", get(documents::list_documents))
        .route("/documents/:id", get(documents::get_document))
        .route("/documents/:id", delete(documents::delete_document))
        // Ingestion - with larger body limit for file uploads
        .route(
            "/ingest",
            post(ingest::ingest_files).layer(DefaultBodyLimit::max(max_upload_size)),
        )
        // Query
        .route("/query", post(query::query_rag))
        // Info
        .route("/info", get(info))
}

/// API info endpoint
async fn info() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "name": "ruvector-rag",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "RAG system with document ingestion and citation-aware answers",
        "endpoints": {
            "POST /api/ingest": "Upload and process documents",
            "POST /api/query": "Query with citations",
            "GET /api/documents": "List all documents",
            "GET /api/documents/:id": "Get document details",
            "DELETE /api/documents/:id": "Delete a document"
        }
    }))
}
