//! Job management and progress endpoints

use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::processing::{FileData, FileError, Job, ProcessingOptions};
use crate::server::state::AppState;

/// Response from async ingest
#[derive(Debug, Serialize)]
pub struct AsyncIngestResponse {
    pub job_id: Uuid,
    pub files_queued: usize,
    pub message: String,
}

/// POST /api/ingest/async - Upload files for async processing
pub async fn ingest_async(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<AsyncIngestResponse>> {
    let mut files = Vec::new();
    let mut options = ProcessingOptions::default();
    options.parallel_embeddings = num_cpus::get().min(8);

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
                options.chunk_size = opts.chunk_size;
                options.chunk_overlap = opts.chunk_overlap;
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
            Ok(d) => d.to_vec(),
            Err(e) => {
                tracing::warn!("Failed to read file {}: {}", filename, e);
                continue;
            }
        };

        tracing::info!("Queued file: {} ({} bytes)", filename, data.len());
        files.push(FileData { filename, data });
    }

    if files.is_empty() {
        return Err(Error::Internal("No files provided".to_string()));
    }

    let files_count = files.len();

    // Create and submit job
    let job = Job {
        id: Uuid::new_v4(),
        files,
        options,
    };

    let job_id = state.job_queue().submit(job).await;

    Ok(Json(AsyncIngestResponse {
        job_id,
        files_queued: files_count,
        message: format!("Job queued successfully. Use /api/jobs/{} to check progress.", job_id),
    }))
}

/// GET /api/jobs/:id - Get job progress
pub async fn get_job_progress(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobProgressResponse>> {
    let progress = state
        .job_queue()
        .get_progress(job_id)
        .ok_or_else(|| Error::DocumentNotFound(format!("Job {} not found", job_id)))?;

    let file_errors: Vec<FileErrorResponse> = progress
        .file_errors
        .iter()
        .map(|e| FileErrorResponse {
            filename: e.filename.clone(),
            error: e.error.clone(),
            stage: format!("{:?}", e.stage).to_lowercase(),
        })
        .collect();

    Ok(Json(JobProgressResponse {
        job_id: progress.job_id,
        status: format!("{:?}", progress.status).to_lowercase(),
        stage: format!("{:?}", progress.stage).to_lowercase(),
        percent_complete: progress.percent_complete(),
        total_files: progress.total_files,
        files_processed: progress.files_processed,
        files_skipped: progress.files_skipped,
        files_failed: progress.files_failed,
        current_file: progress.current_file,
        total_chunks: progress.total_chunks,
        chunks_embedded: progress.chunks_embedded,
        error: progress.error,
        file_errors,
        skipped_files: progress.skipped_files,
        created_at: progress.created_at.to_rfc3339(),
        updated_at: progress.updated_at.to_rfc3339(),
    }))
}

/// GET /api/jobs - List all jobs
pub async fn list_jobs(
    State(state): State<AppState>,
) -> Json<JobListResponse> {
    let jobs = state.job_queue().list_jobs();
    let stats = state.job_queue().stats();

    let jobs: Vec<JobSummary> = jobs
        .into_iter()
        .map(|p| JobSummary {
            job_id: p.job_id,
            status: format!("{:?}", p.status).to_lowercase(),
            stage: format!("{:?}", p.stage).to_lowercase(),
            percent_complete: p.percent_complete(),
            total_files: p.total_files,
            files_processed: p.files_processed,
            error: p.error,
        })
        .collect();

    Json(JobListResponse {
        jobs,
        total_jobs: stats.total_jobs,
        pending: stats.pending,
        processing: stats.processing,
        complete: stats.complete,
        failed: stats.failed,
        worker_count: stats.worker_count,
    })
}

#[derive(Debug, Serialize)]
pub struct JobProgressResponse {
    pub job_id: Uuid,
    pub status: String,
    pub stage: String,
    pub percent_complete: f32,
    pub total_files: usize,
    pub files_processed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub current_file: Option<String>,
    pub total_chunks: usize,
    pub chunks_embedded: usize,
    pub error: Option<String>,
    pub file_errors: Vec<FileErrorResponse>,
    pub skipped_files: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct FileErrorResponse {
    pub filename: String,
    pub error: String,
    pub stage: String,
}

#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub job_id: Uuid,
    pub status: String,
    pub stage: String,
    pub percent_complete: f32,
    pub total_files: usize,
    pub files_processed: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobSummary>,
    pub total_jobs: usize,
    pub pending: usize,
    pub processing: usize,
    pub complete: usize,
    pub failed: usize,
    pub worker_count: usize,
}

#[derive(Debug, Deserialize)]
struct IngestOptions {
    chunk_size: Option<usize>,
    chunk_overlap: Option<usize>,
}
