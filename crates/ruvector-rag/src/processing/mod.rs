//! Background processing with job queue and progress tracking

mod job_queue;
mod worker;

pub use job_queue::{
    FileData, Job, JobQueue, JobProgress, JobStatus, ProcessingOptions, ProcessingStage, QueueStats,
};
pub use worker::ProcessingWorker;
