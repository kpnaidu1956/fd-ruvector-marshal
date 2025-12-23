//! Job queue for background document processing

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Processing stage
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStage {
    Queued,
    Uploading,
    Parsing,
    Chunking,
    Embedding,
    Storing,
    Complete,
    Failed,
}

/// Job status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Processing,
    Complete,
    Failed,
}

/// Error details for a file that failed to process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileError {
    pub filename: String,
    pub error: String,
    pub stage: ProcessingStage,
}

/// Progress information for a job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub job_id: Uuid,
    pub status: JobStatus,
    pub stage: ProcessingStage,
    pub total_files: usize,
    pub files_processed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub current_file: Option<String>,
    pub total_chunks: usize,
    pub chunks_embedded: usize,
    pub error: Option<String>,
    pub file_errors: Vec<FileError>,
    pub skipped_files: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl JobProgress {
    pub fn new(job_id: Uuid, total_files: usize) -> Self {
        let now = chrono::Utc::now();
        Self {
            job_id,
            status: JobStatus::Pending,
            stage: ProcessingStage::Queued,
            total_files,
            files_processed: 0,
            files_skipped: 0,
            files_failed: 0,
            current_file: None,
            total_chunks: 0,
            chunks_embedded: 0,
            error: None,
            file_errors: Vec::new(),
            skipped_files: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn percent_complete(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }

        let file_progress = self.files_processed as f32 / self.total_files as f32;

        // If we're embedding, factor in chunk progress
        if self.stage == ProcessingStage::Embedding && self.total_chunks > 0 {
            let chunk_progress = self.chunks_embedded as f32 / self.total_chunks as f32;
            let current_file_weight = 1.0 / self.total_files as f32;
            return (file_progress + chunk_progress * current_file_weight) * 100.0;
        }

        file_progress * 100.0
    }
}

/// A processing job
#[derive(Debug, Clone)]
pub struct Job {
    pub id: Uuid,
    pub files: Vec<FileData>,
    pub options: ProcessingOptions,
}

/// File data for processing
#[derive(Debug, Clone)]
pub struct FileData {
    pub filename: String,
    pub data: Vec<u8>,
}

/// Processing options
#[derive(Debug, Clone, Default)]
pub struct ProcessingOptions {
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub parallel_embeddings: usize,
}

impl Default for Job {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            files: Vec::new(),
            options: ProcessingOptions::default(),
        }
    }
}

/// Job queue for managing background processing
pub struct JobQueue {
    /// Active jobs with progress
    jobs: Arc<DashMap<Uuid, JobProgress>>,
    /// Channel for sending jobs to workers
    sender: mpsc::Sender<Job>,
    /// Number of workers
    worker_count: usize,
    /// Jobs in queue
    queue_size: Arc<AtomicUsize>,
}

impl JobQueue {
    /// Create a new job queue
    pub fn new(worker_count: usize) -> (Self, mpsc::Receiver<Job>) {
        let (sender, receiver) = mpsc::channel(1000);

        let queue = Self {
            jobs: Arc::new(DashMap::new()),
            sender,
            worker_count,
            queue_size: Arc::new(AtomicUsize::new(0)),
        };

        (queue, receiver)
    }

    /// Submit a job for processing
    pub async fn submit(&self, job: Job) -> Uuid {
        let job_id = job.id;
        let total_files = job.files.len();

        // Create progress entry
        let progress = JobProgress::new(job_id, total_files);
        self.jobs.insert(job_id, progress);
        self.queue_size.fetch_add(1, Ordering::SeqCst);

        // Send to workers
        if let Err(e) = self.sender.send(job).await {
            tracing::error!("Failed to submit job: {}", e);
            self.update_status(job_id, JobStatus::Failed, Some(e.to_string()));
        }

        job_id
    }

    /// Get job progress
    pub fn get_progress(&self, job_id: Uuid) -> Option<JobProgress> {
        self.jobs.get(&job_id).map(|p| p.clone())
    }

    /// Get all jobs
    pub fn list_jobs(&self) -> Vec<JobProgress> {
        self.jobs.iter().map(|e| e.value().clone()).collect()
    }

    /// Update job stage
    pub fn update_stage(&self, job_id: Uuid, stage: ProcessingStage) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.stage = stage;
            progress.updated_at = chrono::Utc::now();
            if stage == ProcessingStage::Complete {
                progress.status = JobStatus::Complete;
                self.queue_size.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Update job status
    pub fn update_status(&self, job_id: Uuid, status: JobStatus, error: Option<String>) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.status = status;
            progress.error = error;
            progress.updated_at = chrono::Utc::now();
            if status == JobStatus::Failed || status == JobStatus::Complete {
                self.queue_size.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    /// Update current file being processed
    pub fn update_current_file(&self, job_id: Uuid, filename: &str) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.current_file = Some(filename.to_string());
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Increment files processed
    pub fn increment_files_processed(&self, job_id: Uuid) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.files_processed += 1;
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Set total chunks for current file
    pub fn set_total_chunks(&self, job_id: Uuid, total: usize) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.total_chunks = total;
            progress.chunks_embedded = 0;
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Increment chunks embedded
    pub fn increment_chunks_embedded(&self, job_id: Uuid, count: usize) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.chunks_embedded += count;
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Add a file error
    pub fn add_file_error(&self, job_id: Uuid, filename: &str, error: &str, stage: ProcessingStage) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.files_failed += 1;
            progress.file_errors.push(FileError {
                filename: filename.to_string(),
                error: error.to_string(),
                stage,
            });
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Add a skipped file
    pub fn add_skipped_file(&self, job_id: Uuid, filename: &str, reason: &str) {
        if let Some(mut progress) = self.jobs.get_mut(&job_id) {
            progress.files_skipped += 1;
            progress.skipped_files.push(format!("{}: {}", filename, reason));
            progress.updated_at = chrono::Utc::now();
        }
    }

    /// Get queue statistics
    pub fn stats(&self) -> QueueStats {
        let total = self.jobs.len();
        let pending = self.jobs.iter().filter(|j| j.status == JobStatus::Pending).count();
        let processing = self.jobs.iter().filter(|j| j.status == JobStatus::Processing).count();
        let complete = self.jobs.iter().filter(|j| j.status == JobStatus::Complete).count();
        let failed = self.jobs.iter().filter(|j| j.status == JobStatus::Failed).count();

        QueueStats {
            total_jobs: total,
            pending,
            processing,
            complete,
            failed,
            worker_count: self.worker_count,
        }
    }

    /// Get jobs reference for workers
    pub fn jobs_ref(&self) -> Arc<DashMap<Uuid, JobProgress>> {
        self.jobs.clone()
    }
}

/// Queue statistics
#[derive(Debug, Clone, Serialize)]
pub struct QueueStats {
    pub total_jobs: usize,
    pub pending: usize,
    pub processing: usize,
    pub complete: usize,
    pub failed: usize,
    pub worker_count: usize,
}
