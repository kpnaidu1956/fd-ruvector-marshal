//! Bucket-based file storage API endpoints
//!
//! Provides generic file storage for frontend attachments (task files, etc.)
//! Separate from document ingestion - these files are stored but not processed for RAG.

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::server::state::AppState;

/// Allowed storage buckets for frontend files
const ALLOWED_BUCKETS: &[&str] = &[
    "task-attachments",
    "user-avatars",
    "organization-logos",
    "message-attachments",
    "goal-attachments",
];

/// Maximum file size for storage uploads (50 MB)
const MAX_STORAGE_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Response for file upload
#[derive(Debug, Serialize)]
pub struct StorageUploadResponse {
    pub success: bool,
    pub url: String,
    pub path: String,
    pub size: u64,
    pub content_type: String,
}

/// Response for file deletion
#[derive(Debug, Serialize)]
pub struct StorageDeleteResponse {
    pub success: bool,
    pub message: String,
}

/// Validate bucket name
fn validate_bucket(bucket: &str) -> Result<()> {
    if !ALLOWED_BUCKETS.contains(&bucket) {
        return Err(Error::Validation(format!(
            "Invalid bucket '{}'. Allowed buckets: {}",
            bucket,
            ALLOWED_BUCKETS.join(", ")
        )));
    }
    Ok(())
}

/// Validate and sanitize storage path
fn validate_storage_path(path: &str) -> Result<String> {
    // Prevent path traversal attacks
    if path.contains("..") || path.starts_with('/') {
        return Err(Error::Validation("Invalid path: contains forbidden characters".to_string()));
    }

    // Limit path depth
    let depth = path.matches('/').count();
    if depth > 5 {
        return Err(Error::Validation("Path too deep (max 5 levels)".to_string()));
    }

    // Limit total path length
    if path.len() > 500 {
        return Err(Error::Validation("Path too long (max 500 characters)".to_string()));
    }

    Ok(path.to_string())
}

/// POST /api/storage/upload - Upload a file to storage bucket
///
/// Request: multipart/form-data with fields:
/// - bucket: Storage bucket name (e.g., "task-attachments")
/// - path: Path within bucket (e.g., "tasks/{task_id}/{filename}")
/// - file: The file binary
///
/// Response:
/// ```json
/// {
///   "success": true,
///   "url": "https://rags.goalign.ai/api/storage/task-attachments/tasks/abc123/document.pdf",
///   "path": "tasks/abc123/document.pdf",
///   "size": 1024000,
///   "content_type": "application/pdf"
/// }
/// ```
#[cfg(feature = "gcp")]
pub async fn upload_storage_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<StorageUploadResponse>> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut bucket: Option<String> = None;
    let mut path: Option<String> = None;
    let mut content_type: Option<String> = None;

    // Parse multipart form
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        Error::Internal(format!("Failed to read multipart field: {}", e))
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                // Get content type from field
                if content_type.is_none() {
                    content_type = field.content_type().map(|s| s.to_string());
                }
                file_data = Some(field.bytes().await.map_err(|e| {
                    Error::Internal(format!("Failed to read file data: {}", e))
                })?.to_vec());
            }
            "bucket" => {
                bucket = Some(field.text().await.map_err(|e| {
                    Error::Internal(format!("Failed to read bucket: {}", e))
                })?);
            }
            "path" => {
                path = Some(field.text().await.map_err(|e| {
                    Error::Internal(format!("Failed to read path: {}", e))
                })?);
            }
            _ => {}
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        Error::Validation("Missing 'file' in multipart form".to_string())
    })?;
    let bucket = bucket.ok_or_else(|| {
        Error::Validation("Missing 'bucket' in multipart form".to_string())
    })?;
    let path = path.ok_or_else(|| {
        Error::Validation("Missing 'path' in multipart form".to_string())
    })?;

    // Validate inputs
    validate_bucket(&bucket)?;
    let validated_path = validate_storage_path(&path)?;

    let file_size = file_data.len() as u64;
    if file_size > MAX_STORAGE_FILE_SIZE {
        return Err(Error::Validation(format!(
            "File size ({} MB) exceeds maximum allowed ({} MB)",
            file_size / (1024 * 1024),
            MAX_STORAGE_FILE_SIZE / (1024 * 1024)
        )));
    }

    if file_size == 0 {
        return Err(Error::Validation("File is empty".to_string()));
    }

    // Determine content type
    let content_type = content_type.unwrap_or_else(|| {
        mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string()
    });

    // Get GCS document store
    let document_store = state.document_store()
        .ok_or_else(|| Error::Internal("GCS document store not available".to_string()))?;

    // Build GCS path: storage/{bucket}/{path}
    let gcs_path = format!("storage/{}/{}", bucket, validated_path);

    // Upload to GCS
    document_store.store_storage_file(&gcs_path, &file_data, &content_type).await?;

    // Build public URL
    let public_url = format!(
        "https://rags.goalign.ai/api/storage/{}/{}",
        bucket, validated_path
    );

    tracing::info!(
        "Stored file in bucket '{}': {} ({} bytes)",
        bucket, validated_path, file_size
    );

    Ok(Json(StorageUploadResponse {
        success: true,
        url: public_url,
        path: validated_path,
        size: file_size,
        content_type,
    }))
}

/// GET /api/storage/:bucket/*path - Download a file from storage
#[cfg(feature = "gcp")]
pub async fn download_storage_file(
    State(state): State<AppState>,
    Path((bucket, path)): Path<(String, String)>,
) -> Result<Response> {
    // Validate inputs
    validate_bucket(&bucket)?;
    let validated_path = validate_storage_path(&path)?;

    // Get GCS document store
    let document_store = state.document_store()
        .ok_or_else(|| Error::Internal("GCS document store not available".to_string()))?;

    // Build GCS path
    let gcs_path = format!("storage/{}/{}", bucket, validated_path);

    // Download from GCS
    let (data, content_type) = document_store.get_storage_file(&gcs_path).await?;

    // Get filename from path for content-disposition
    let filename = validated_path.rsplit('/').next().unwrap_or("file");

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", filename)
        )
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .body(Body::from(data))
        .map_err(|e| Error::Internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// DELETE /api/storage/:bucket/*path - Delete a file from storage
#[cfg(feature = "gcp")]
pub async fn delete_storage_file(
    State(state): State<AppState>,
    Path((bucket, path)): Path<(String, String)>,
) -> Result<Json<StorageDeleteResponse>> {
    // Validate inputs
    validate_bucket(&bucket)?;
    let validated_path = validate_storage_path(&path)?;

    // Get GCS document store
    let document_store = state.document_store()
        .ok_or_else(|| Error::Internal("GCS document store not available".to_string()))?;

    // Build GCS path
    let gcs_path = format!("storage/{}/{}", bucket, validated_path);

    // Delete from GCS
    document_store.delete_storage_file(&gcs_path).await?;

    tracing::info!("Deleted file from bucket '{}': {}", bucket, validated_path);

    Ok(Json(StorageDeleteResponse {
        success: true,
        message: format!("File deleted: {}/{}", bucket, validated_path),
    }))
}

// Non-GCP stubs
#[cfg(not(feature = "gcp"))]
pub async fn upload_storage_file(
    _state: State<AppState>,
    _multipart: Multipart,
) -> Result<Json<StorageUploadResponse>> {
    Err(Error::Internal("Storage API requires GCP feature".to_string()))
}

#[cfg(not(feature = "gcp"))]
pub async fn download_storage_file(
    _state: State<AppState>,
    _path: Path<(String, String)>,
) -> Result<Response> {
    Err(Error::Internal("Storage API requires GCP feature".to_string()))
}

#[cfg(not(feature = "gcp"))]
pub async fn delete_storage_file(
    _state: State<AppState>,
    _path: Path<(String, String)>,
) -> Result<Json<StorageDeleteResponse>> {
    Err(Error::Internal("Storage API requires GCP feature".to_string()))
}
