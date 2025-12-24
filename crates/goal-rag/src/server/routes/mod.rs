//! API routes for the RAG server

pub mod documents;
pub mod ingest;
pub mod jobs;
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
        // Async ingestion with progress tracking
        .route(
            "/ingest/async",
            post(jobs::ingest_async).layer(DefaultBodyLimit::max(max_upload_size)),
        )
        // Job management
        .route("/jobs", get(jobs::list_jobs))
        .route("/jobs/:id", get(jobs::get_job_progress))
        // Query
        .route("/query", post(query::query_rag))
        // V2 Query (frontend-friendly format)
        .route("/v2/query", post(query::query_rag_v2))
        // String search
        .route("/string-search", post(query::string_search))
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
            "POST /api/ingest": "Upload and process documents (sync)",
            "POST /api/ingest/async": "Upload documents for async processing",
            "GET /api/jobs": "List all jobs and queue stats",
            "GET /api/jobs/:id": "Get job progress",
            "POST /api/query": "Query with citations (v1)",
            "POST /api/v2/query": "Query with citations (v2 - frontend-friendly format)",
            "POST /api/string-search": "Literal string search",
            "GET /api/documents": "List all documents",
            "GET /api/documents/:id": "Get document details",
            "DELETE /api/documents/:id": "Delete a document"
        },
        "features": {
            "gcs_storage": "Original files and plain text stored in GCS",
            "deduplication": "Content-hash based file deduplication",
            "string_search": "Literal text search for words/phrases",
            "answer_caching": "Cached answers with document-based invalidation",
            "grounded_answers": "LLM uses only document content, no external knowledge"
        }
    }))
}
