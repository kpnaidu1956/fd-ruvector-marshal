//! SQLite storage for analytics data
//!
//! Stores interaction classifications, workflow timelines, patterns, and recommendations.

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{Error, Result};
use super::types::*;

/// Analytics database for storing classifications, timelines, patterns
pub struct AnalyticsDb {
    conn: Arc<Mutex<Connection>>,
}

impl AnalyticsDb {
    /// Create or open the analytics database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| Error::Internal(format!("Failed to open analytics database: {}", e)))?;

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

        // Enable WAL mode for better concurrency
        conn.execute_batch(r#"
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=10000;
            PRAGMA temp_store=MEMORY;
        "#).map_err(|e| Error::Internal(format!("Failed to set pragmas: {}", e)))?;

        conn.execute_batch(r#"
            -- Interaction classifications table
            CREATE TABLE IF NOT EXISTS interaction_classifications (
                id TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                task_id TEXT,
                goal_id TEXT,
                sender_id TEXT NOT NULL,
                content TEXT NOT NULL,

                interaction_type TEXT NOT NULL,
                secondary_types TEXT,
                confidence_score REAL NOT NULL,
                entities TEXT,
                sentiment REAL,
                urgency_level TEXT,

                references_interaction_id TEXT,
                original_created_at TEXT NOT NULL,
                classified_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_ic_org ON interaction_classifications(organization_id);
            CREATE INDEX IF NOT EXISTS idx_ic_task ON interaction_classifications(task_id);
            CREATE INDEX IF NOT EXISTS idx_ic_goal ON interaction_classifications(goal_id);
            CREATE INDEX IF NOT EXISTS idx_ic_type ON interaction_classifications(interaction_type);
            CREATE INDEX IF NOT EXISTS idx_ic_source ON interaction_classifications(source_type, source_id);

            -- Workflow timelines table
            CREATE TABLE IF NOT EXISTS workflow_timelines (
                id TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,

                total_interactions INTEGER,
                total_participants INTEGER,
                total_duration_hours REAL,

                phases TEXT NOT NULL,
                key_events TEXT NOT NULL,
                bottlenecks TEXT,

                status TEXT NOT NULL,
                opened_at TEXT NOT NULL,
                closed_at TEXT,
                last_analyzed_at TEXT NOT NULL,

                UNIQUE(entity_type, entity_id)
            );

            CREATE INDEX IF NOT EXISTS idx_wt_org ON workflow_timelines(organization_id);
            CREATE INDEX IF NOT EXISTS idx_wt_entity ON workflow_timelines(entity_type, entity_id);

            -- Workflow patterns table
            CREATE TABLE IF NOT EXISTS workflow_patterns (
                id TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                pattern_name TEXT NOT NULL,
                description TEXT NOT NULL,
                criteria TEXT NOT NULL,

                occurrence_count INTEGER,
                success_correlation REAL,
                avg_time_impact_hours REAL,
                confidence_score REAL NOT NULL,

                examples TEXT NOT NULL,
                is_active INTEGER DEFAULT 1,

                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,

                UNIQUE(organization_id, pattern_name)
            );

            CREATE INDEX IF NOT EXISTS idx_wp_org ON workflow_patterns(organization_id);
            CREATE INDEX IF NOT EXISTS idx_wp_type ON workflow_patterns(pattern_type);

            -- Efficiency recommendations table
            CREATE TABLE IF NOT EXISTS efficiency_recommendations (
                id TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT,

                recommendation_type TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                suggested_actions TEXT NOT NULL,

                based_on_patterns TEXT NOT NULL,
                evidence TEXT NOT NULL,

                priority TEXT NOT NULL,
                estimated_time_savings_hours REAL,

                status TEXT DEFAULT 'pending',
                user_feedback TEXT,
                generated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_er_org ON efficiency_recommendations(organization_id);
            CREATE INDEX IF NOT EXISTS idx_er_target ON efficiency_recommendations(target_type, target_id);
            CREATE INDEX IF NOT EXISTS idx_er_status ON efficiency_recommendations(status);

            -- Analysis jobs table
            CREATE TABLE IF NOT EXISTS analysis_jobs (
                id TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,

                status TEXT NOT NULL,
                progress_percent INTEGER DEFAULT 0,
                current_stage TEXT,

                interactions_found INTEGER DEFAULT 0,
                interactions_classified INTEGER DEFAULT 0,
                patterns_matched INTEGER DEFAULT 0,
                recommendations_generated INTEGER DEFAULT 0,

                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_aj_org ON analysis_jobs(organization_id);
            CREATE INDEX IF NOT EXISTS idx_aj_entity ON analysis_jobs(entity_type, entity_id);
            CREATE INDEX IF NOT EXISTS idx_aj_status ON analysis_jobs(status);
        "#)
        .map_err(|e| Error::Internal(format!("Failed to run analytics migrations: {}", e)))?;

        tracing::info!("Analytics database migrations complete");
        Ok(())
    }

    // ==================== Interaction Classifications ====================

    /// Insert a classification
    pub fn insert_classification(&self, classification: &InteractionClassification) -> Result<()> {
        let conn = self.conn.lock();

        let secondary_types_json = serde_json::to_string(&classification.secondary_types)
            .unwrap_or_else(|_| "[]".to_string());
        let entities_json = serde_json::to_string(&classification.entities)
            .unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            r#"
            INSERT INTO interaction_classifications (
                id, organization_id, source_type, source_id, task_id, goal_id,
                sender_id, content, interaction_type, secondary_types, confidence_score,
                entities, sentiment, urgency_level, references_interaction_id,
                original_created_at, classified_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
            params![
                classification.id.to_string(),
                classification.organization_id,
                classification.source_type.as_str(),
                classification.source_id,
                classification.task_id,
                classification.goal_id,
                classification.sender_id,
                classification.content,
                classification.interaction_type.as_str(),
                secondary_types_json,
                classification.confidence_score,
                entities_json,
                classification.sentiment,
                classification.urgency_level.as_str(),
                classification.references_interaction_id,
                classification.original_created_at.to_rfc3339(),
                classification.classified_at.to_rfc3339(),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to insert classification: {}", e)))?;

        Ok(())
    }

    /// Get classifications for a task
    pub fn get_classifications_for_task(&self, task_id: &str) -> Result<Vec<InteractionClassification>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM interaction_classifications WHERE task_id = ?1 ORDER BY original_created_at ASC"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![task_id], row_to_classification)
            .map_err(|e| Error::Internal(format!("Failed to query classifications: {}", e)))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Failed to parse classification row: {}", e);
                    None
                }
            })
            .collect();

        Ok(records)
    }

    /// Get classifications for a goal
    pub fn get_classifications_for_goal(&self, goal_id: &str) -> Result<Vec<InteractionClassification>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM interaction_classifications WHERE goal_id = ?1 ORDER BY original_created_at ASC"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![goal_id], row_to_classification)
            .map_err(|e| Error::Internal(format!("Failed to query classifications: {}", e)))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Failed to parse classification row: {}", e);
                    None
                }
            })
            .collect();

        Ok(records)
    }

    /// Search classifications by type
    pub fn search_classifications_by_type(
        &self,
        organization_id: &str,
        interaction_type: InteractionType,
        limit: usize,
    ) -> Result<Vec<InteractionClassification>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM interaction_classifications WHERE organization_id = ?1 AND interaction_type = ?2 ORDER BY classified_at DESC LIMIT ?3"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(
            params![organization_id, interaction_type.as_str(), limit as i64],
            row_to_classification,
        )
        .map_err(|e| Error::Internal(format!("Failed to query classifications: {}", e)))?
        .filter_map(|r| match r {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Failed to parse classification row: {}", e);
                None
            }
        })
        .collect();

        Ok(records)
    }

    /// Check if a source has already been classified
    pub fn is_source_classified(&self, source_type: &str, source_id: &str) -> Result<bool> {
        let conn = self.conn.lock();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM interaction_classifications WHERE source_type = ?1 AND source_id = ?2",
            params![source_type, source_id],
            |row| row.get(0),
        ).map_err(|e| Error::Internal(format!("Failed to check classification: {}", e)))?;

        Ok(count > 0)
    }

    // ==================== Workflow Timelines ====================

    /// Upsert a workflow timeline
    pub fn upsert_timeline(&self, timeline: &WorkflowTimeline) -> Result<()> {
        let conn = self.conn.lock();

        let phases_json = serde_json::to_string(&timeline.phases)
            .unwrap_or_else(|_| "[]".to_string());
        let events_json = serde_json::to_string(&timeline.key_events)
            .unwrap_or_else(|_| "[]".to_string());
        let bottlenecks_json = serde_json::to_string(&timeline.bottlenecks)
            .unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            r#"
            INSERT INTO workflow_timelines (
                id, organization_id, entity_type, entity_id,
                total_interactions, total_participants, total_duration_hours,
                phases, key_events, bottlenecks,
                status, opened_at, closed_at, last_analyzed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                total_interactions = excluded.total_interactions,
                total_participants = excluded.total_participants,
                total_duration_hours = excluded.total_duration_hours,
                phases = excluded.phases,
                key_events = excluded.key_events,
                bottlenecks = excluded.bottlenecks,
                status = excluded.status,
                closed_at = excluded.closed_at,
                last_analyzed_at = excluded.last_analyzed_at
            "#,
            params![
                timeline.id.to_string(),
                timeline.organization_id,
                timeline.entity_type,
                timeline.entity_id,
                timeline.total_interactions,
                timeline.total_participants,
                timeline.total_duration_hours,
                phases_json,
                events_json,
                bottlenecks_json,
                timeline.status,
                timeline.opened_at.to_rfc3339(),
                timeline.closed_at.map(|t| t.to_rfc3339()),
                timeline.last_analyzed_at.to_rfc3339(),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to upsert timeline: {}", e)))?;

        Ok(())
    }

    /// Get timeline for an entity
    pub fn get_timeline(&self, entity_type: &str, entity_id: &str) -> Result<Option<WorkflowTimeline>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM workflow_timelines WHERE entity_type = ?1 AND entity_id = ?2"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let record = stmt.query_row(params![entity_type, entity_id], row_to_timeline)
            .optional()
            .map_err(|e| Error::Internal(format!("Failed to query timeline: {}", e)))?;

        Ok(record)
    }

    // ==================== Workflow Patterns ====================

    /// Upsert a pattern
    pub fn upsert_pattern(&self, pattern: &WorkflowPattern) -> Result<()> {
        let conn = self.conn.lock();

        let criteria_json = serde_json::to_string(&pattern.criteria)
            .unwrap_or_else(|_| "{}".to_string());
        let examples_json = serde_json::to_string(&pattern.examples)
            .unwrap_or_else(|_| "[]".to_string());

        conn.execute(
            r#"
            INSERT INTO workflow_patterns (
                id, organization_id, pattern_type, pattern_name, description, criteria,
                occurrence_count, success_correlation, avg_time_impact_hours, confidence_score,
                examples, is_active, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(organization_id, pattern_name) DO UPDATE SET
                description = excluded.description,
                criteria = excluded.criteria,
                occurrence_count = excluded.occurrence_count,
                success_correlation = excluded.success_correlation,
                avg_time_impact_hours = excluded.avg_time_impact_hours,
                confidence_score = excluded.confidence_score,
                examples = excluded.examples,
                updated_at = excluded.updated_at
            "#,
            params![
                pattern.id.to_string(),
                pattern.organization_id,
                pattern.pattern_type.as_str(),
                pattern.pattern_name,
                pattern.description,
                criteria_json,
                pattern.occurrence_count,
                pattern.success_correlation,
                pattern.avg_time_impact_hours,
                pattern.confidence_score,
                examples_json,
                pattern.is_active as i32,
                pattern.created_at.to_rfc3339(),
                pattern.updated_at.to_rfc3339(),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to upsert pattern: {}", e)))?;

        Ok(())
    }

    /// Get all active patterns for an organization
    pub fn get_patterns(&self, organization_id: &str) -> Result<Vec<WorkflowPattern>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM workflow_patterns WHERE organization_id = ?1 AND is_active = 1 ORDER BY confidence_score DESC"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![organization_id], row_to_pattern)
            .map_err(|e| Error::Internal(format!("Failed to query patterns: {}", e)))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Failed to parse pattern row: {}", e);
                    None
                }
            })
            .collect();

        Ok(records)
    }

    // ==================== Recommendations ====================

    /// Insert a recommendation
    pub fn insert_recommendation(&self, rec: &EfficiencyRecommendation) -> Result<()> {
        let conn = self.conn.lock();

        let actions_json = serde_json::to_string(&rec.suggested_actions)
            .unwrap_or_else(|_| "[]".to_string());
        let patterns_json = serde_json::to_string(&rec.based_on_patterns)
            .unwrap_or_else(|_| "[]".to_string());
        let evidence_json = serde_json::to_string(&rec.evidence)
            .unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            r#"
            INSERT INTO efficiency_recommendations (
                id, organization_id, target_type, target_id,
                recommendation_type, title, description, suggested_actions,
                based_on_patterns, evidence, priority, estimated_time_savings_hours,
                status, user_feedback, generated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                rec.id.to_string(),
                rec.organization_id,
                rec.target_type.as_str(),
                rec.target_id,
                rec.recommendation_type.as_str(),
                rec.title,
                rec.description,
                actions_json,
                patterns_json,
                evidence_json,
                rec.priority.as_str(),
                rec.estimated_time_savings_hours,
                rec.status.as_str(),
                rec.user_feedback,
                rec.generated_at.to_rfc3339(),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to insert recommendation: {}", e)))?;

        Ok(())
    }

    /// Get recommendations for a target
    pub fn get_recommendations_for_target(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> Result<Vec<EfficiencyRecommendation>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM efficiency_recommendations WHERE target_type = ?1 AND target_id = ?2 ORDER BY generated_at DESC"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![target_type, target_id], row_to_recommendation)
            .map_err(|e| Error::Internal(format!("Failed to query recommendations: {}", e)))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Failed to parse recommendation row: {}", e);
                    None
                }
            })
            .collect();

        Ok(records)
    }

    /// Get org-wide recommendations
    pub fn get_org_recommendations(&self, organization_id: &str, limit: usize) -> Result<Vec<EfficiencyRecommendation>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM efficiency_recommendations WHERE organization_id = ?1 AND status = 'pending' ORDER BY priority DESC, generated_at DESC LIMIT ?2"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let records = stmt.query_map(params![organization_id, limit as i64], row_to_recommendation)
            .map_err(|e| Error::Internal(format!("Failed to query recommendations: {}", e)))?
            .filter_map(|r| match r {
                Ok(v) => Some(v),
                Err(e) => {
                    tracing::warn!("Failed to parse recommendation row: {}", e);
                    None
                }
            })
            .collect();

        Ok(records)
    }

    /// Update recommendation status with feedback
    pub fn update_recommendation_feedback(
        &self,
        id: &Uuid,
        status: RecommendationStatus,
        feedback: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock();

        let count = conn.execute(
            "UPDATE efficiency_recommendations SET status = ?2, user_feedback = ?3 WHERE id = ?1",
            params![id.to_string(), status.as_str(), feedback],
        ).map_err(|e| Error::Internal(format!("Failed to update recommendation: {}", e)))?;

        Ok(count > 0)
    }

    // ==================== Analysis Jobs ====================

    /// Create an analysis job
    pub fn create_analysis_job(&self, job: &AnalysisJob) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute(
            r#"
            INSERT INTO analysis_jobs (
                id, organization_id, entity_type, entity_id,
                status, progress_percent, current_stage,
                interactions_found, interactions_classified, patterns_matched, recommendations_generated,
                error, created_at, updated_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                job.id.to_string(),
                job.organization_id,
                job.entity_type,
                job.entity_id,
                job.status.as_str(),
                job.progress_percent as i32,
                job.current_stage,
                job.interactions_found,
                job.interactions_classified,
                job.patterns_matched,
                job.recommendations_generated,
                job.error,
                job.created_at.to_rfc3339(),
                job.updated_at.to_rfc3339(),
                job.completed_at.map(|t| t.to_rfc3339()),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to create analysis job: {}", e)))?;

        Ok(())
    }

    /// Update analysis job progress
    pub fn update_analysis_job(&self, job: &AnalysisJob) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute(
            r#"
            UPDATE analysis_jobs SET
                status = ?2,
                progress_percent = ?3,
                current_stage = ?4,
                interactions_found = ?5,
                interactions_classified = ?6,
                patterns_matched = ?7,
                recommendations_generated = ?8,
                error = ?9,
                updated_at = ?10,
                completed_at = ?11
            WHERE id = ?1
            "#,
            params![
                job.id.to_string(),
                job.status.as_str(),
                job.progress_percent as i32,
                job.current_stage,
                job.interactions_found,
                job.interactions_classified,
                job.patterns_matched,
                job.recommendations_generated,
                job.error,
                job.updated_at.to_rfc3339(),
                job.completed_at.map(|t| t.to_rfc3339()),
            ],
        ).map_err(|e| Error::Internal(format!("Failed to update analysis job: {}", e)))?;

        Ok(())
    }

    /// Get analysis job by ID
    pub fn get_analysis_job(&self, id: &Uuid) -> Result<Option<AnalysisJob>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT * FROM analysis_jobs WHERE id = ?1"
        ).map_err(|e| Error::Internal(format!("Failed to prepare query: {}", e)))?;

        let record = stmt.query_row(params![id.to_string()], row_to_analysis_job)
            .optional()
            .map_err(|e| Error::Internal(format!("Failed to query analysis job: {}", e)))?;

        Ok(record)
    }
}

// ==================== Row Converters ====================

fn row_to_classification(row: &rusqlite::Row) -> rusqlite::Result<InteractionClassification> {
    let id_str: String = row.get(0)?;
    let organization_id: String = row.get(1)?;
    let source_type_str: String = row.get(2)?;
    let source_id: String = row.get(3)?;
    let task_id: Option<String> = row.get(4)?;
    let goal_id: Option<String> = row.get(5)?;
    let sender_id: String = row.get(6)?;
    let content: String = row.get(7)?;
    let interaction_type_str: String = row.get(8)?;
    let secondary_types_json: Option<String> = row.get(9)?;
    let confidence_score: f64 = row.get(10)?;
    let entities_json: Option<String> = row.get(11)?;
    let sentiment: Option<f64> = row.get(12)?;
    let urgency_str: Option<String> = row.get(13)?;
    let references_id: Option<String> = row.get(14)?;
    let original_at_str: String = row.get(15)?;
    let classified_at_str: String = row.get(16)?;

    Ok(InteractionClassification {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        organization_id,
        source_type: InteractionSource::from_str(&source_type_str),
        source_id,
        task_id,
        goal_id,
        sender_id,
        content,
        interaction_type: InteractionType::from_str(&interaction_type_str),
        secondary_types: secondary_types_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        confidence_score: confidence_score as f32,
        entities: entities_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        sentiment: sentiment.unwrap_or(0.0) as f32,
        urgency_level: urgency_str.map(|s| UrgencyLevel::from_str(&s)).unwrap_or(UrgencyLevel::Medium),
        references_interaction_id: references_id,
        original_created_at: DateTime::parse_from_rfc3339(&original_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        classified_at: DateTime::parse_from_rfc3339(&classified_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_timeline(row: &rusqlite::Row) -> rusqlite::Result<WorkflowTimeline> {
    let id_str: String = row.get(0)?;
    let organization_id: String = row.get(1)?;
    let entity_type: String = row.get(2)?;
    let entity_id: String = row.get(3)?;
    let total_interactions: Option<i32> = row.get(4)?;
    let total_participants: Option<i32> = row.get(5)?;
    let total_duration: Option<f64> = row.get(6)?;
    let phases_json: String = row.get(7)?;
    let events_json: String = row.get(8)?;
    let bottlenecks_json: Option<String> = row.get(9)?;
    let status: String = row.get(10)?;
    let opened_at_str: String = row.get(11)?;
    let closed_at_str: Option<String> = row.get(12)?;
    let analyzed_at_str: String = row.get(13)?;

    Ok(WorkflowTimeline {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        organization_id,
        entity_type,
        entity_id,
        total_interactions: total_interactions.unwrap_or(0).max(0) as u32,
        total_participants: total_participants.unwrap_or(0).max(0) as u32,
        total_duration_hours: total_duration,
        phases: serde_json::from_str(&phases_json).unwrap_or_default(),
        key_events: serde_json::from_str(&events_json).unwrap_or_default(),
        bottlenecks: bottlenecks_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default(),
        status,
        opened_at: DateTime::parse_from_rfc3339(&opened_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        closed_at: closed_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).ok()
        }),
        last_analyzed_at: DateTime::parse_from_rfc3339(&analyzed_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_pattern(row: &rusqlite::Row) -> rusqlite::Result<WorkflowPattern> {
    let id_str: String = row.get(0)?;
    let organization_id: String = row.get(1)?;
    let pattern_type_str: String = row.get(2)?;
    let pattern_name: String = row.get(3)?;
    let description: String = row.get(4)?;
    let criteria_json: String = row.get(5)?;
    let occurrence_count: Option<i32> = row.get(6)?;
    let success_correlation: Option<f64> = row.get(7)?;
    let avg_time_impact: Option<f64> = row.get(8)?;
    let confidence_score: f64 = row.get(9)?;
    let examples_json: String = row.get(10)?;
    let is_active: i32 = row.get(11)?;
    let created_at_str: String = row.get(12)?;
    let updated_at_str: String = row.get(13)?;

    Ok(WorkflowPattern {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        organization_id,
        pattern_type: PatternType::from_str(&pattern_type_str),
        pattern_name,
        description,
        criteria: serde_json::from_str(&criteria_json).unwrap_or(serde_json::json!({})),
        occurrence_count: occurrence_count.unwrap_or(0).max(0) as u32,
        success_correlation: success_correlation.map(|s| (s as f32).clamp(-1.0, 1.0)),
        avg_time_impact_hours: avg_time_impact,
        confidence_score: (confidence_score as f32).clamp(0.0, 1.0),
        examples: serde_json::from_str(&examples_json).unwrap_or_default(),
        is_active: is_active != 0,
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_recommendation(row: &rusqlite::Row) -> rusqlite::Result<EfficiencyRecommendation> {
    let id_str: String = row.get(0)?;
    let organization_id: String = row.get(1)?;
    let target_type_str: String = row.get(2)?;
    let target_id: Option<String> = row.get(3)?;
    let rec_type_str: String = row.get(4)?;
    let title: String = row.get(5)?;
    let description: String = row.get(6)?;
    let actions_json: String = row.get(7)?;
    let patterns_json: String = row.get(8)?;
    let evidence_json: String = row.get(9)?;
    let priority_str: String = row.get(10)?;
    let time_savings: Option<f64> = row.get(11)?;
    let status_str: String = row.get(12)?;
    let user_feedback: Option<String> = row.get(13)?;
    let generated_at_str: String = row.get(14)?;

    Ok(EfficiencyRecommendation {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        organization_id,
        target_type: RecommendationTarget::from_str(&target_type_str),
        target_id,
        recommendation_type: RecommendationType::from_str(&rec_type_str),
        title,
        description,
        suggested_actions: serde_json::from_str(&actions_json).unwrap_or_default(),
        based_on_patterns: serde_json::from_str(&patterns_json).unwrap_or_default(),
        evidence: serde_json::from_str(&evidence_json).unwrap_or(serde_json::json!({})),
        priority: UrgencyLevel::from_str(&priority_str),
        estimated_time_savings_hours: time_savings,
        status: RecommendationStatus::from_str(&status_str),
        user_feedback,
        generated_at: DateTime::parse_from_rfc3339(&generated_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_analysis_job(row: &rusqlite::Row) -> rusqlite::Result<AnalysisJob> {
    let id_str: String = row.get(0)?;
    let organization_id: String = row.get(1)?;
    let entity_type: String = row.get(2)?;
    let entity_id: String = row.get(3)?;
    let status_str: String = row.get(4)?;
    let progress: i32 = row.get(5)?;
    let current_stage: Option<String> = row.get(6)?;
    let interactions_found: i32 = row.get(7)?;
    let interactions_classified: i32 = row.get(8)?;
    let patterns_matched: i32 = row.get(9)?;
    let recommendations_generated: i32 = row.get(10)?;
    let error: Option<String> = row.get(11)?;
    let created_at_str: String = row.get(12)?;
    let updated_at_str: String = row.get(13)?;
    let completed_at_str: Option<String> = row.get(14)?;

    // Clamp progress to valid u8 range (0-100 for percentage)
    let progress_clamped = progress.clamp(0, 100) as u8;

    Ok(AnalysisJob {
        id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
        organization_id,
        entity_type,
        entity_id,
        status: AnalysisJobStatus::from_str(&status_str),
        progress_percent: progress_clamped,
        current_stage: current_stage.unwrap_or_else(|| "unknown".to_string()),
        interactions_found: interactions_found.max(0) as u32,
        interactions_classified: interactions_classified.max(0) as u32,
        patterns_matched: patterns_matched.max(0) as u32,
        recommendations_generated: recommendations_generated.max(0) as u32,
        error,
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        completed_at: completed_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).ok()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_insert_and_query() {
        let db = AnalyticsDb::in_memory().unwrap();

        let classification = InteractionClassification {
            id: Uuid::new_v4(),
            organization_id: "test-org".to_string(),
            source_type: InteractionSource::TaskComment,
            source_id: "comment-123".to_string(),
            task_id: Some("task-456".to_string()),
            goal_id: None,
            sender_id: "user-789".to_string(),
            content: "Can you clarify the requirements?".to_string(),
            interaction_type: InteractionType::RequestClarification,
            secondary_types: vec![InteractionType::Question],
            confidence_score: 0.92,
            entities: ExtractedEntities::default(),
            sentiment: 0.1,
            urgency_level: UrgencyLevel::Medium,
            references_interaction_id: None,
            original_created_at: Utc::now(),
            classified_at: Utc::now(),
        };

        db.insert_classification(&classification).unwrap();

        let results = db.get_classifications_for_task("task-456").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].interaction_type, InteractionType::RequestClarification);
    }
}
