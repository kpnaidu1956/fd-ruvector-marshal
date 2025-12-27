//! Storage module for persistent data storage
//!
//! Provides SQLite-based persistence for file registry and documents.

mod database;

pub use database::{
    FileRegistryDb, FileRegistryDbStats, SyncStatus,
    // Job persistence types
    JobFileRecord, JobFileStatus, JobOptions, JobRecord, PersistedJobStage, PersistedJobStatus,
    // Chunk content types (for FTS)
    ChunkContentRecord, ChunkSearchResult,
};
