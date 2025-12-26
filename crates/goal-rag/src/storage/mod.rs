//! Storage module for persistent data storage
//!
//! Provides SQLite-based persistence for file registry and documents.

mod database;

pub use database::{FileRegistryDb, FileRegistryDbStats, SyncStatus};
