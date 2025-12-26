//! File status and tracking API endpoints

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::server::state::{AppState, FileRegistryStats};
use crate::storage::SyncStatus;
use crate::types::{
    FileCheckItem, FileCheckRequest, FileCheckResponse, FileCheckResult, FileCheckSummary,
    FileRecord, FileRecordStatus, FileRecordSummary, FileUploadAdvice,
};

/// Query parameters for listing files
#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    /// Filter by status: success, failed, skipped, all
    #[serde(default = "default_status")]
    pub status: String,
    /// Limit results
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
    /// Sort by: filename, date, size
    #[serde(default = "default_sort")]
    pub sort: String,
    /// Sort order: asc, desc
    #[serde(default = "default_order")]
    pub order: String,
}

fn default_status() -> String {
    "all".to_string()
}

fn default_limit() -> usize {
    100
}

fn default_sort() -> String {
    "date".to_string()
}

fn default_order() -> String {
    "desc".to_string()
}

/// Response for file list
#[derive(Debug, Serialize)]
pub struct FileListResponse {
    /// Files in the list
    pub files: Vec<FileRecordSummary>,
    /// Total count (before pagination)
    pub total: usize,
    /// Current offset
    pub offset: usize,
    /// Current limit
    pub limit: usize,
    /// Statistics
    pub stats: FileRegistryStats,
}

/// Response for failed files list
#[derive(Debug, Serialize)]
pub struct FailedFilesResponse {
    /// Failed files with details
    pub files: Vec<FailedFileDetail>,
    /// Total count
    pub total: usize,
    /// Suggestions for fixing
    pub suggestions: Vec<String>,
}

/// Detail for a failed file
#[derive(Debug, Serialize)]
pub struct FailedFileDetail {
    pub filename: String,
    pub file_type: String,
    pub file_size: u64,
    pub error_message: String,
    pub failed_at_stage: String,
    pub last_attempt: String,
    pub upload_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

/// GET /api/files - List all tracked files
pub async fn list_files(
    State(state): State<AppState>,
    Query(params): Query<ListFilesQuery>,
) -> Json<FileListResponse> {
    let mut records: Vec<FileRecord> = match params.status.as_str() {
        "success" => state.list_successful_files(),
        "failed" => state.list_failed_files(),
        "skipped" => state.list_skipped_files(),
        _ => state.list_file_records(),
    };

    let total = records.len();

    // Sort
    match params.sort.as_str() {
        "filename" => records.sort_by(|a, b| a.filename.cmp(&b.filename)),
        "size" => records.sort_by(|a, b| a.file_size.cmp(&b.file_size)),
        _ => records.sort_by(|a, b| b.last_processed_at.cmp(&a.last_processed_at)),
    }

    if params.order == "asc" {
        records.reverse();
    }

    // Paginate
    let records: Vec<FileRecordSummary> = records
        .into_iter()
        .skip(params.offset)
        .take(params.limit)
        .map(|r| FileRecordSummary::from(&r))
        .collect();

    let stats = state.file_registry_stats();

    Json(FileListResponse {
        files: records,
        total,
        offset: params.offset,
        limit: params.limit,
        stats,
    })
}

/// GET /api/files/:filename - Get specific file status
pub async fn get_file_status(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Json<FileRecord>> {
    state
        .get_file_record(&filename)
        .map(Json)
        .ok_or_else(|| Error::DocumentNotFound(format!("File '{}' not found in registry", filename)))
}

/// POST /api/files/check - Check status of files before upload
pub async fn check_files(
    State(state): State<AppState>,
    Json(request): Json<FileCheckRequest>,
) -> Json<FileCheckResponse> {
    let mut results = Vec::new();
    let mut needs_upload = 0;
    let mut can_skip = 0;
    let mut should_retry = 0;
    let total_checked = request.files.len();

    for item in request.files {
        let (advice, existing_record) = check_single_file(&state, &item);

        match &advice {
            FileUploadAdvice::Upload => needs_upload += 1,
            FileUploadAdvice::Skip { .. } => can_skip += 1,
            FileUploadAdvice::Retry { .. } => should_retry += 1,
        }

        results.push(FileCheckResult {
            filename: item.filename,
            advice,
            existing_record,
        });
    }

    Json(FileCheckResponse {
        files: results,
        summary: FileCheckSummary {
            total_checked,
            needs_upload,
            can_skip,
            should_retry,
        },
    })
}

/// Check a single file for upload status
fn check_single_file(state: &AppState, item: &FileCheckItem) -> (FileUploadAdvice, Option<FileRecordSummary>) {
    // First check if we have a record by filename
    if let Some(record) = state.get_file_record(&item.filename) {
        let summary = FileRecordSummary::from(&record);

        match record.status {
            FileRecordStatus::Success => {
                // Check if content hash matches (if provided)
                if let Some(ref hash) = item.content_hash {
                    if &record.content_hash == hash {
                        // Same content, skip
                        return (
                            FileUploadAdvice::Skip {
                                reason: "File unchanged (same content hash)".to_string(),
                                existing_document_id: record.document_id,
                            },
                            Some(summary),
                        );
                    } else {
                        // Content changed, upload for update
                        return (FileUploadAdvice::Upload, Some(summary));
                    }
                }

                // No hash provided, check file size
                if record.file_size == item.file_size {
                    return (
                        FileUploadAdvice::Skip {
                            reason: "File unchanged (same size)".to_string(),
                            existing_document_id: record.document_id,
                        },
                        Some(summary),
                    );
                }

                // Different size, likely modified
                (FileUploadAdvice::Upload, Some(summary))
            }
            FileRecordStatus::Failed => {
                // Previously failed, recommend retry
                (
                    FileUploadAdvice::Retry {
                        previous_error: record.error_message.unwrap_or_else(|| "Unknown error".to_string()),
                    },
                    Some(summary),
                )
            }
            FileRecordStatus::Skipped => {
                // Was skipped (duplicate), skip again unless content changed
                if let Some(ref hash) = item.content_hash {
                    if &record.content_hash != hash {
                        return (FileUploadAdvice::Upload, Some(summary));
                    }
                }
                (
                    FileUploadAdvice::Skip {
                        reason: "Previously skipped (duplicate)".to_string(),
                        existing_document_id: record.document_id,
                    },
                    Some(summary),
                )
            }
            FileRecordStatus::Processing => {
                // Currently being processed
                (
                    FileUploadAdvice::Skip {
                        reason: "Currently being processed".to_string(),
                        existing_document_id: None,
                    },
                    Some(summary),
                )
            }
        }
    } else if let Some(ref hash) = item.content_hash {
        // No record by filename, check by content hash
        if let Some(record) = state.get_file_record_by_hash(hash) {
            let summary = FileRecordSummary::from(&record);
            return (
                FileUploadAdvice::Skip {
                    reason: format!("Same content exists as '{}'", record.filename),
                    existing_document_id: record.document_id,
                },
                Some(summary),
            );
        }
        // New file
        (FileUploadAdvice::Upload, None)
    } else {
        // New file (no record found)
        (FileUploadAdvice::Upload, None)
    }
}

/// GET /api/files/failed - List failed files with details
pub async fn list_failed_files(
    State(state): State<AppState>,
) -> Json<FailedFilesResponse> {
    let failed = state.list_failed_files();
    let total = failed.len();

    let files: Vec<FailedFileDetail> = failed
        .iter()
        .map(|record| {
            let suggested_action = suggest_action_for_failure(
                &record.error_message.clone().unwrap_or_default(),
                &record.failed_at_stage.clone().unwrap_or_default(),
            );

            FailedFileDetail {
                filename: record.filename.clone(),
                file_type: record.file_type.display_name().to_string(),
                file_size: record.file_size,
                error_message: record.error_message.clone().unwrap_or_else(|| "Unknown error".to_string()),
                failed_at_stage: record.failed_at_stage.clone().unwrap_or_else(|| "unknown".to_string()),
                last_attempt: record.last_processed_at.to_rfc3339(),
                upload_count: record.upload_count,
                suggested_action,
            }
        })
        .collect();

    // Generate suggestions based on error patterns
    let mut suggestions = Vec::new();
    let error_messages: Vec<&str> = files.iter()
        .map(|f| f.error_message.as_str())
        .collect();

    if error_messages.iter().any(|e| e.contains("OCR") || e.contains("tesseract")) {
        suggestions.push("Install tesseract-ocr for better scanned document support".to_string());
    }
    if error_messages.iter().any(|e| e.contains("LibreOffice") || e.contains("libreoffice")) {
        suggestions.push("Install LibreOffice for legacy .doc/.ppt/.xls support".to_string());
    }
    if error_messages.iter().any(|e| e.contains("timeout")) {
        suggestions.push("Some files may be too large or complex. Try splitting large documents.".to_string());
    }
    if error_messages.iter().any(|e| e.contains("No text content")) {
        suggestions.push("Some documents may be image-only. Ensure OCR tools are installed.".to_string());
    }
    if error_messages.iter().any(|e| e.contains("rate limit") || e.contains("429")) {
        suggestions.push("Rate limiting detected. Processing will resume automatically.".to_string());
    }

    Json(FailedFilesResponse {
        files,
        total,
        suggestions,
    })
}

/// Suggest action based on error message
fn suggest_action_for_failure(error: &str, stage: &str) -> Option<String> {
    let error_lower = error.to_lowercase();

    if error_lower.contains("no text content") || error_lower.contains("empty") {
        Some("Document may be image-based. Ensure OCR is installed and retry.".to_string())
    } else if error_lower.contains("timeout") {
        Some("Document took too long to process. Try splitting into smaller files.".to_string())
    } else if error_lower.contains("unsupported") || error_lower.contains("unknown file type") {
        Some("Convert to a supported format (PDF, DOCX, XLSX, TXT).".to_string())
    } else if error_lower.contains("libreoffice") {
        Some("Install LibreOffice or convert to modern Office format.".to_string())
    } else if error_lower.contains("ocr") || error_lower.contains("tesseract") {
        Some("Install tesseract-ocr or use a non-scanned version.".to_string())
    } else if error_lower.contains("corrupt") || error_lower.contains("invalid") {
        Some("File may be corrupted. Try re-saving or exporting to a new file.".to_string())
    } else if error_lower.contains("password") || error_lower.contains("encrypted") {
        Some("Remove password protection from the document.".to_string())
    } else if stage == "embedding" {
        Some("Embedding service error. Retry upload - transient issue.".to_string())
    } else {
        None
    }
}

/// DELETE /api/files/failed - Clear all failed file records
pub async fn clear_failed_files(
    State(state): State<AppState>,
) -> Json<ClearFailedResponse> {
    let cleared = state.clear_failed_files();
    Json(ClearFailedResponse {
        cleared,
        message: format!("Cleared {} failed file records. You can now retry uploading these files.", cleared),
    })
}

#[derive(Debug, Serialize)]
pub struct ClearFailedResponse {
    pub cleared: usize,
    pub message: String,
}

/// DELETE /api/files/:filename - Remove a specific file record
pub async fn delete_file_record(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Json<DeleteFileResponse>> {
    match state.remove_file_record(&filename) {
        Some(record) => Ok(Json(DeleteFileResponse {
            filename: record.filename,
            message: "File record removed. You can re-upload this file.".to_string(),
        })),
        None => Err(Error::DocumentNotFound(format!("File '{}' not found in registry", filename))),
    }
}

#[derive(Debug, Serialize)]
pub struct DeleteFileResponse {
    pub filename: String,
    pub message: String,
}

/// GET /api/files/stats - Get file registry statistics
pub async fn file_stats(
    State(state): State<AppState>,
) -> Json<FileStatsResponse> {
    let stats = state.file_registry_stats();
    let failed = state.list_failed_files();

    // Group failures by error type
    let mut error_types: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for record in &failed {
        let error = record.error_message.clone().unwrap_or_else(|| "Unknown".to_string());
        let error_type = categorize_error(&error);
        *error_types.entry(error_type).or_insert(0) += 1;
    }

    Json(FileStatsResponse {
        total_files: stats.total,
        successful: stats.success,
        failed: stats.failed,
        skipped: stats.skipped,
        success_rate: if stats.total > 0 {
            (stats.success as f32 / stats.total as f32) * 100.0
        } else {
            0.0
        },
        error_breakdown: error_types,
    })
}

fn categorize_error(error: &str) -> String {
    let error_lower = error.to_lowercase();
    if error_lower.contains("timeout") {
        "Timeout".to_string()
    } else if error_lower.contains("no text") || error_lower.contains("empty") {
        "Empty/No Text".to_string()
    } else if error_lower.contains("ocr") || error_lower.contains("tesseract") {
        "OCR Required".to_string()
    } else if error_lower.contains("libreoffice") {
        "LibreOffice Required".to_string()
    } else if error_lower.contains("unsupported") || error_lower.contains("unknown") {
        "Unsupported Format".to_string()
    } else if error_lower.contains("rate") || error_lower.contains("429") {
        "Rate Limited".to_string()
    } else if error_lower.contains("embedding") {
        "Embedding Error".to_string()
    } else {
        "Other".to_string()
    }
}

#[derive(Debug, Serialize)]
pub struct FileStatsResponse {
    pub total_files: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub success_rate: f32,
    pub error_breakdown: std::collections::HashMap<String, usize>,
}

// ============================================================================
// GCS Sync Endpoints
// ============================================================================

/// POST /api/files/sync - Sync file registry from GCS bucket
#[cfg(feature = "gcp")]
pub async fn sync_from_gcs(
    State(state): State<AppState>,
) -> Result<Json<SyncResponse>> {
    let (synced, failed) = state.sync_from_gcs().await?;

    Ok(Json(SyncResponse {
        success: true,
        files_synced: synced,
        files_failed: failed,
        message: format!(
            "Synced {} files from GCS bucket ({} failed)",
            synced, failed
        ),
        sync_status: state.get_sync_status(),
    }))
}

/// GET /api/files/sync/status - Get last sync status
pub async fn get_sync_status(
    State(state): State<AppState>,
) -> Json<SyncStatusResponse> {
    let sync_status = state.get_sync_status();
    let db_stats = state.database_stats();

    Json(SyncStatusResponse {
        last_sync: sync_status,
        database_stats: db_stats,
    })
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub success: bool,
    pub files_synced: usize,
    pub files_failed: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_status: Option<SyncStatus>,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<SyncStatus>,
    pub database_stats: crate::storage::FileRegistryDbStats,
}

/// GET /api/files/gcs-counts - Get file counts from GCS bucket
#[cfg(feature = "gcp")]
pub async fn get_gcs_counts(
    State(state): State<AppState>,
) -> Result<Json<GcsCountsResponse>> {
    let document_store = state.document_store()
        .ok_or_else(|| Error::Internal("GCS document store not available".to_string()))?;

    let (originals, plaintext) = document_store.get_file_counts().await?;

    Ok(Json(GcsCountsResponse {
        originals_count: originals,
        plaintext_count: plaintext,
        failed_estimate: originals.saturating_sub(plaintext),
    }))
}

#[derive(Debug, Serialize)]
pub struct GcsCountsResponse {
    /// Number of original files in GCS
    pub originals_count: usize,
    /// Number of plaintext files in GCS (successfully processed)
    pub plaintext_count: usize,
    /// Estimated number of failed files (originals without plaintext)
    pub failed_estimate: usize,
}
