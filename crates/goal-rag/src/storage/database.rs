//! SQLite database for persistent file registry storage
//!
//! Provides durable storage for file processing status, replacing JSON file storage.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{Connection, params, OptionalExtension};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::types::{FileRecord, FileRecordStatus, FileType};

/// SQLite-based file registry database
pub struct FileRegistryDb {
    conn: Arc<Mutex<Connection>>,
}

impl FileRegistryDb {
    /// Create or open the database at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| Error::Internal(format!("Failed to open database: {}", e)))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.migrate()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| Error::Internal(format!("Failed to open in-memory database: {}", e)))?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.migrate()?;
        Ok(db)
    }

    /// Run database migrations
    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(r#"
            -- File registry table
            CREATE TABLE IF NOT EXISTS file_registry (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL UNIQUE,
                content_hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                file_type TEXT NOT NULL,
                status TEXT NOT NULL,
                document_id TEXT,
                chunks_created INTEGER,
                skip_reason TEXT,
                error_message TEXT,
                failed_at_stage TEXT,
                job_id TEXT,
                first_seen_at TEXT NOT NULL,
                last_processed_at TEXT NOT NULL,
                upload_count INTEGER NOT NULL DEFAULT 1,
                original_url TEXT,
                plaintext_url TEXT,
                gcs_synced INTEGER NOT NULL DEFAULT 0
            );

            -- Index for efficient lookups
            CREATE INDEX IF NOT EXISTS idx_file_registry_status ON file_registry(status);
            CREATE INDEX IF NOT EXISTS idx_file_registry_content_hash ON file_registry(content_hash);
            CREATE INDEX IF NOT EXISTS idx_file_registry_document_id ON file_registry(document_id);

            -- Documents table
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                internal_filename TEXT,
                file_type TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                total_chunks INTEGER,
                total_pages INTEGER,
                ingested_at TEXT NOT NULL,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_documents_filename ON documents(filename);
            CREATE INDEX IF NOT EXISTS idx_documents_content_hash ON documents(content_hash);

            -- Sync status table
            CREATE TABLE IF NOT EXISTS sync_status (
                id INTEGER PRIMARY KEY,
                last_gcs_sync TEXT,
                files_synced INTEGER DEFAULT 0,
                sync_duration_ms INTEGER
            );

            -- Initialize sync status if not exists
            INSERT OR IGNORE INTO sync_status (id, last_gcs_sync, files_synced) VALUES (1, NULL, 0);
        "#)
        .map_err(|e| Error::Internal(format!("Failed to run migrations: {}", e)))?;

        tracing::info!("Database migrations complete");
        Ok(())
    }

    // ==================== File Registry Operations ====================

    /// Insert or update a file record
    pub fn upsert_file_record(&self, record: &FileRecord) -> Result<()> {
        let conn = self.conn.lock();

        let skip_reason_json = record.skip_reason.as_ref()
            .map(|r| serde_json::to_string(r).unwrap_or_default());

        conn.execute(
            r#"
            INSERT INTO file_registry (
                id, filename, content_hash, file_size, file_type, status,
                document_id, chunks_created, skip_reason, error_message, failed_at_stage,
                job_id, first_seen_at, last_processed_at, upload_count, original_url, plaintext_url
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(filename) DO UPDATE SET
                content_hash = excluded.content_hash,
                file_size = excluded.file_size,
                file_type = excluded.file_type,
                status = excluded.status,
                document_id = excluded.document_id,
                chunks_created = excluded.chunks_created,
                skip_reason = excluded.skip_reason,
                error_message = excluded.error_message,
                failed_at_stage = excluded.failed_at_stage,
                job_id = excluded.job_id,
                last_processed_at = excluded.last_processed_at,
                upload_count = file_registry.upload_count + 1,
                original_url = COALESCE(excluded.original_url, file_registry.original_url),
                plaintext_url = COALESCE(excluded.plaintext_url, file_registry.plaintext_url)
            "#,
            params![
                record.id.to_string(),
                record.filename,
                record.content_hash,
                record.file_size as i64,
                file_type_to_extension(&record.file_type),
                status_to_string(&record.status),
                record.document_id.map(|id| id.to_string()),
                record.chunks_created.map(|c| c as i64),
                skip_reason_json,
                record.error_message,
                record.failed_at_stage,
                record.job_id.map(|id| id.to_string()),
                record.first_seen_at.to_rfc3339(),
                record.last_processed_at.to_rfc3339(),
                record.upload_count as i64,
                record.original_url,
                record.plaintext_url,
            ],
        ).map_err(|e| Error::Internal(format!("Failed to upsert file record: {}", e)))?;

        Ok(())
    }

    /// Get a file record by filename
    pub fn get_file_record(&self, filename: &str) -> Result<Option<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM file_registry WHERE filename = ?1"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let record = stmt.query_row(params![filename], |row| {
            row_to_file_record(row)
        }).optional()
        .map_err(|e| Error::Internal(format!("Failed to get file record: {}", e)))?;

        Ok(record)
    }

    /// Get a file record by content hash
    pub fn get_file_record_by_hash(&self, content_hash: &str) -> Result<Option<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM file_registry WHERE content_hash = ?1 LIMIT 1"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let record = stmt.query_row(params![content_hash], |row| {
            row_to_file_record(row)
        }).optional()
        .map_err(|e| Error::Internal(format!("Failed to get file record: {}", e)))?;

        Ok(record)
    }

    /// List all file records
    pub fn list_file_records(&self) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare("SELECT * FROM file_registry ORDER BY last_processed_at DESC")
            .map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map([], row_to_file_record)
            .map_err(|e| Error::Internal(format!("Failed to list file records: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// List file records by status
    pub fn list_by_status(&self, status: FileRecordStatus) -> Result<Vec<FileRecord>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM file_registry WHERE status = ?1 ORDER BY last_processed_at DESC"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![status_to_string(&status)], row_to_file_record)
            .map_err(|e| Error::Internal(format!("Failed to list file records: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(records)
    }

    /// Delete a file record
    pub fn delete_file_record(&self, filename: &str) -> Result<bool> {
        let conn = self.conn.lock();

        let count = conn.execute(
            "DELETE FROM file_registry WHERE filename = ?1",
            params![filename],
        ).map_err(|e| Error::Internal(format!("Failed to delete file record: {}", e)))?;

        Ok(count > 0)
    }

    /// Clear all failed file records
    pub fn clear_failed_files(&self) -> Result<usize> {
        let conn = self.conn.lock();

        let count = conn.execute(
            "DELETE FROM file_registry WHERE status = 'failed'",
            [],
        ).map_err(|e| Error::Internal(format!("Failed to clear failed files: {}", e)))?;

        Ok(count)
    }

    /// Get file registry statistics
    pub fn get_stats(&self) -> Result<FileRegistryDbStats> {
        let conn = self.conn.lock();

        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_registry",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let success: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_registry WHERE status = 'success'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let failed: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_registry WHERE status = 'failed'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let skipped: i64 = conn.query_row(
            "SELECT COUNT(*) FROM file_registry WHERE status = 'skipped'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(FileRegistryDbStats {
            total: total as usize,
            success: success as usize,
            failed: failed as usize,
            skipped: skipped as usize,
        })
    }

    // ==================== GCS Sync Operations ====================

    /// Record a file discovered from GCS sync
    pub fn sync_from_gcs(
        &self,
        filename: &str,
        document_id: Uuid,
        content_hash: &str,
        file_size: u64,
        file_type: &str,
        has_plaintext: bool,
        original_url: &str,
        plaintext_url: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();

        let status = if has_plaintext { "success" } else { "failed" };
        let error_message = if has_plaintext { None } else {
            Some("No plaintext found in GCS - processing may have failed".to_string())
        };

        let now = Utc::now().to_rfc3339();

        conn.execute(
            r#"
            INSERT INTO file_registry (
                id, filename, content_hash, file_size, file_type, status,
                document_id, chunks_created, error_message, first_seen_at,
                last_processed_at, upload_count, original_url, plaintext_url, gcs_synced
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, ?12, ?13, 1)
            ON CONFLICT(filename) DO UPDATE SET
                document_id = COALESCE(excluded.document_id, file_registry.document_id),
                original_url = COALESCE(excluded.original_url, file_registry.original_url),
                plaintext_url = COALESCE(excluded.plaintext_url, file_registry.plaintext_url),
                gcs_synced = 1
            "#,
            params![
                document_id.to_string(),
                filename,
                content_hash,
                file_size as i64,
                file_type,
                status,
                document_id.to_string(),
                if has_plaintext { Some(0i64) } else { None },  // chunks_created unknown from GCS
                error_message,
                &now,
                &now,
                original_url,
                plaintext_url,
            ],
        ).map_err(|e| Error::Internal(format!("Failed to sync from GCS: {}", e)))?;

        Ok(())
    }

    /// Update last GCS sync timestamp
    pub fn update_sync_status(&self, files_synced: usize, duration_ms: u64) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE sync_status SET last_gcs_sync = ?1, files_synced = ?2, sync_duration_ms = ?3 WHERE id = 1",
            params![Utc::now().to_rfc3339(), files_synced as i64, duration_ms as i64],
        ).map_err(|e| Error::Internal(format!("Failed to update sync status: {}", e)))?;

        Ok(())
    }

    /// Get last sync status
    pub fn get_sync_status(&self) -> Result<Option<SyncStatus>> {
        let conn = self.conn.lock();

        let status = conn.query_row(
            "SELECT last_gcs_sync, files_synced, sync_duration_ms FROM sync_status WHERE id = 1",
            [],
            |row| {
                let last_sync: Option<String> = row.get(0)?;
                let files_synced: i64 = row.get(1)?;
                let duration_ms: Option<i64> = row.get(2)?;

                Ok(SyncStatus {
                    last_gcs_sync: last_sync.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                    files_synced: files_synced as usize,
                    sync_duration_ms: duration_ms.map(|d| d as u64),
                })
            },
        ).optional()
        .map_err(|e| Error::Internal(format!("Failed to get sync status: {}", e)))?;

        Ok(status)
    }
}

/// Database statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRegistryDbStats {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped: usize,
}

/// GCS sync status
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub last_gcs_sync: Option<DateTime<Utc>>,
    pub files_synced: usize,
    pub sync_duration_ms: Option<u64>,
}

// Helper functions

fn status_to_string(status: &FileRecordStatus) -> &'static str {
    match status {
        FileRecordStatus::Success => "success",
        FileRecordStatus::Skipped => "skipped",
        FileRecordStatus::Failed => "failed",
        FileRecordStatus::Processing => "processing",
    }
}

fn string_to_status(s: &str) -> FileRecordStatus {
    match s {
        "success" => FileRecordStatus::Success,
        "skipped" => FileRecordStatus::Skipped,
        "failed" => FileRecordStatus::Failed,
        "processing" => FileRecordStatus::Processing,
        _ => FileRecordStatus::Failed,
    }
}

fn file_type_to_extension(file_type: &FileType) -> &'static str {
    match file_type {
        FileType::Pdf => "pdf",
        FileType::Docx => "docx",
        FileType::Doc => "doc",
        FileType::Pptx => "pptx",
        FileType::Ppt => "ppt",
        FileType::Txt => "txt",
        FileType::Markdown => "md",
        FileType::Xlsx => "xlsx",
        FileType::Xls => "xls",
        FileType::Html => "html",
        FileType::Csv => "csv",
        FileType::Rtf => "rtf",
        FileType::Odt => "odt",
        FileType::Odp => "odp",
        FileType::Ods => "ods",
        FileType::Epub => "epub",
        FileType::Image => "image",
        FileType::Code(_) => "code",
        FileType::Unknown => "unknown",
    }
}

fn row_to_file_record(row: &rusqlite::Row) -> rusqlite::Result<FileRecord> {
    let id_str: String = row.get(0)?;
    let filename: String = row.get(1)?;
    let content_hash: String = row.get(2)?;
    let file_size: i64 = row.get(3)?;
    let file_type_str: String = row.get(4)?;
    let status_str: String = row.get(5)?;
    let document_id_str: Option<String> = row.get(6)?;
    let chunks_created: Option<i64> = row.get(7)?;
    let skip_reason_json: Option<String> = row.get(8)?;
    let error_message: Option<String> = row.get(9)?;
    let failed_at_stage: Option<String> = row.get(10)?;
    let job_id_str: Option<String> = row.get(11)?;
    let first_seen_at_str: String = row.get(12)?;
    let last_processed_at_str: String = row.get(13)?;
    let upload_count: i64 = row.get(14)?;
    let original_url: Option<String> = row.get(15)?;
    let plaintext_url: Option<String> = row.get(16)?;

    Ok(FileRecord {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        filename,
        content_hash,
        file_size: file_size as u64,
        file_type: FileType::from_extension(&file_type_str),
        status: string_to_status(&status_str),
        document_id: document_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        chunks_created: chunks_created.map(|c| c as u32),
        skip_reason: skip_reason_json.and_then(|j| serde_json::from_str(&j).ok()),
        error_message,
        failed_at_stage,
        job_id: job_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        first_seen_at: DateTime::parse_from_rfc3339(&first_seen_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        last_processed_at: DateTime::parse_from_rfc3339(&last_processed_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        upload_count: upload_count as u32,
        original_url,
        plaintext_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_and_get() {
        let db = FileRegistryDb::in_memory().unwrap();

        let record = FileRecord::success(
            "test.pdf".to_string(),
            "abc123".to_string(),
            1000,
            FileType::Pdf,
            Uuid::new_v4(),
            10,
            None,
        );

        db.upsert_file_record(&record).unwrap();

        let retrieved = db.get_file_record("test.pdf").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().filename, "test.pdf");
    }

    #[test]
    fn test_stats() {
        let db = FileRegistryDb::in_memory().unwrap();

        // Add some records
        db.upsert_file_record(&FileRecord::success(
            "success.pdf".to_string(), "hash1".to_string(), 100, FileType::Pdf, Uuid::new_v4(), 5, None,
        )).unwrap();

        db.upsert_file_record(&FileRecord::failed(
            "failed.pdf".to_string(), "hash2".to_string(), 100, FileType::Pdf, "error".to_string(), "parsing".to_string(), None,
        )).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.success, 1);
        assert_eq!(stats.failed, 1);
    }
}
