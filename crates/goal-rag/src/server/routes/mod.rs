//! API routes for the RAG server

pub mod documents;
pub mod files;
pub mod ingest;
pub mod jobs;
pub mod query;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};
use crate::ingestion::ExternalParser;
use crate::server::state::AppState;

/// Build all API routes
pub fn api_routes(max_upload_size: usize) -> Router<AppState> {
    let router = Router::new()
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
        .route("/jobs/incomplete", get(jobs::list_incomplete_jobs))
        .route("/jobs/:id", get(jobs::get_job_progress))
        .route("/jobs/:id/files", get(jobs::get_job_files_progress))
        .route("/jobs/:id/resume", post(jobs::resume_job))
        // System information
        .route("/system/parsers", get(jobs::get_parsers_status))
        // File status and tracking
        .route("/files", get(files::list_files))
        .route("/files/check", post(files::check_files))
        .route("/files/failed", get(files::list_failed_files))
        .route("/files/failed", delete(files::clear_failed_files))
        .route("/files/stats", get(files::file_stats))
        .route("/files/sync/status", get(files::get_sync_status))
        .route("/files/:filename", get(files::get_file_status))
        .route("/files/:filename", delete(files::delete_file_record))
        // Query
        .route("/query", post(query::query_rag))
        // V2 Query (frontend-friendly format)
        .route("/v2/query", post(query::query_rag_v2))
        // String search
        .route("/string-search", post(query::string_search))
        // Info and capabilities
        .route("/info", get(info))
        .route("/capabilities", get(capabilities));

    // Add GCP-specific routes when gcp feature is enabled
    #[cfg(feature = "gcp")]
    let router = router
        .route("/files/sync", post(files::sync_from_gcs))
        .route("/files/gcs-counts", get(files::get_gcs_counts));

    router
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
            "GET /api/jobs/incomplete": "List incomplete jobs that can be resumed",
            "GET /api/jobs/:id": "Get job progress",
            "GET /api/jobs/:id/files": "Get per-file progress with tier and parser details",
            "POST /api/jobs/:id/resume": "Resume an incomplete/failed job",
            "GET /api/system/parsers": "Get available parsers and their status",
            "POST /api/query": "Query with citations (v1)",
            "POST /api/v2/query": "Query with citations (v2 - frontend-friendly format)",
            "POST /api/string-search": "Literal string search",
            "GET /api/documents": "List all documents",
            "GET /api/documents/:id": "Get document details",
            "DELETE /api/documents/:id": "Delete a document",
            "GET /api/files": "List all tracked files with status",
            "POST /api/files/check": "Check file status before upload (deduplication)",
            "GET /api/files/failed": "List failed files with error details",
            "DELETE /api/files/failed": "Clear all failed file records for retry",
            "GET /api/files/stats": "Get file registry statistics",
            "GET /api/files/:filename": "Get specific file status",
            "DELETE /api/files/:filename": "Remove file record for re-upload",
            "POST /api/files/sync": "Sync file registry from GCS bucket (GCP only)",
            "GET /api/files/sync/status": "Get last GCS sync status",
            "GET /api/files/gcs-counts": "Get file counts from GCS bucket (GCP only)",
            "GET /api/capabilities": "Check document extraction capabilities"
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

/// Document extraction capabilities endpoint
async fn capabilities() -> axum::Json<serde_json::Value> {
    let has_pdftotext = ExternalParser::has_pdftotext();
    let has_tesseract = ExternalParser::has_tesseract();
    let has_pdftoppm = ExternalParser::has_pdftoppm();
    let has_pandoc = ExternalParser::has_pandoc();

    // Check for LibreOffice
    let has_libreoffice = std::process::Command::new("libreoffice")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    axum::Json(serde_json::json!({
        "tools": {
            "pdftotext": {
                "available": has_pdftotext,
                "purpose": "Fast PDF text extraction",
                "install": "apt install poppler-utils"
            },
            "tesseract": {
                "available": has_tesseract,
                "purpose": "OCR for scanned PDFs and images",
                "install": "apt install tesseract-ocr"
            },
            "pdftoppm": {
                "available": has_pdftoppm,
                "purpose": "PDF to image conversion for OCR",
                "install": "apt install poppler-utils"
            },
            "pandoc": {
                "available": has_pandoc,
                "purpose": "Document conversion (DOCX, RTF, EPUB, ODT)",
                "install": "apt install pandoc"
            },
            "libreoffice": {
                "available": has_libreoffice,
                "purpose": "Legacy format conversion (DOC, PPT, XLS)",
                "install": "apt install libreoffice"
            }
        },
        "formats": {
            "pdf": {
                "native": true,
                "enhanced": has_pdftotext,
                "ocr": has_tesseract && has_pdftoppm,
                "status": if has_pdftotext && has_tesseract { "full" } else if has_pdftotext { "good" } else { "basic" }
            },
            "docx": {
                "native": true,
                "fallback": has_pandoc,
                "status": "full"
            },
            "doc": {
                "native": false,
                "conversion": has_libreoffice,
                "fallback": has_pandoc,
                "status": if has_libreoffice || has_pandoc { "available" } else { "unavailable" }
            },
            "pptx": {
                "native": true,
                "fallback": has_pandoc,
                "status": "full"
            },
            "ppt": {
                "native": false,
                "conversion": has_libreoffice,
                "status": if has_libreoffice { "available" } else { "unavailable" }
            },
            "xlsx": {
                "native": true,
                "status": "full"
            },
            "xls": {
                "native": true,
                "fallback": has_libreoffice,
                "status": "full"
            },
            "rtf": {
                "native": false,
                "conversion": has_pandoc || has_libreoffice,
                "status": if has_pandoc || has_libreoffice { "available" } else { "unavailable" }
            },
            "odt": {
                "native": false,
                "conversion": has_pandoc || has_libreoffice,
                "status": if has_pandoc || has_libreoffice { "available" } else { "unavailable" }
            },
            "odp": {
                "native": false,
                "conversion": has_libreoffice,
                "status": if has_libreoffice { "available" } else { "unavailable" }
            },
            "ods": {
                "native": false,
                "conversion": has_libreoffice,
                "status": if has_libreoffice { "available" } else { "unavailable" }
            },
            "epub": {
                "native": false,
                "conversion": has_pandoc,
                "status": if has_pandoc { "available" } else { "unavailable" }
            },
            "images": {
                "native": false,
                "ocr": has_tesseract,
                "formats": ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff"],
                "status": if has_tesseract { "available" } else { "unavailable" }
            },
            "txt": { "native": true, "status": "full" },
            "md": { "native": true, "status": "full" },
            "html": { "native": true, "status": "full" },
            "csv": { "native": true, "status": "full" },
            "code": { "native": true, "status": "full", "extensions": ["rs", "py", "js", "ts", "go", "java", "cpp", "c", "cs", "rb", "php", "swift", "kt", "sql", "sh", "yaml", "json", "xml", "toml"] }
        },
        "recommendations": {
            "for_scanned_pdfs": if !has_tesseract { Some("Install tesseract-ocr for OCR support") } else { None },
            "for_legacy_office": if !has_libreoffice { Some("Install libreoffice for DOC/PPT/XLS support") } else { None },
            "for_better_pdf": if !has_pdftotext { Some("Install poppler-utils for better PDF extraction") } else { None },
            "for_documents": if !has_pandoc { Some("Install pandoc for RTF/EPUB/ODT support") } else { None }
        }
    }))
}
