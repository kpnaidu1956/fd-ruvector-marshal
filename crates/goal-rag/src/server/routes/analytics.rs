//! Analytics API routes
//!
//! Provides endpoints for interaction analysis, timeline reconstruction,
//! pattern learning, and efficiency recommendations.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
#[cfg(feature = "postgres")]
use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::analytics::{
    AnalysisJob, AnalysisJobStatus, InteractionType, RecommendationStatus,
};
use crate::analytics::jobs::{AnalyticsJobProcessor, TaskAnalysisInput};
#[cfg(feature = "postgres")]
use crate::analytics::jobs::{TaskComment, RelatedMessage};
use crate::analytics::storage::AnalyticsDb;
#[cfg(feature = "postgres")]
use crate::analytics::timeline::ActivityEvent;
use crate::server::state::AppState;

// ==================== Request/Response Types ====================

#[derive(Debug, Deserialize)]
pub struct AnalyzeTaskRequest {
    pub organization_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeGoalRequest {
    pub organization_id: String,
}

/// Maximum allowed limit for queries to prevent DoS
const MAX_QUERY_LIMIT: usize = 500;

#[derive(Debug, Serialize)]
pub struct AnalysisJobResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchInteractionsQuery {
    pub organization_id: String,
    #[serde(default)]
    pub interaction_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Clamp limit to safe range
fn sanitize_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_QUERY_LIMIT)
}

/// Validate organization_id is non-empty and reasonable length
fn validate_org_id(org_id: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if org_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "organization_id is required" })),
        ));
    }
    if org_id.len() > 128 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "organization_id exceeds maximum length" })),
        ));
    }
    Ok(())
}

/// Validate entity_type is "task" or "goal"
/// Note: Reserved for future dynamic entity type support
#[allow(dead_code)]
fn validate_entity_type(entity_type: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if entity_type != "task" && entity_type != "goal" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "entity_type must be 'task' or 'goal'" })),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct RecommendationFeedback {
    pub status: String, // "accepted", "rejected", "implemented"
    #[serde(default)]
    pub feedback: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrgRecommendationsQuery {
    pub organization_id: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct PatternLearnRequest {
    pub organization_id: String,
}

// ==================== Handlers ====================

/// Trigger analysis for a task
/// POST /api/analytics/analysis/task/:task_id
pub async fn analyze_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<AnalyzeTaskRequest>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&request.organization_id) {
        return e;
    }
    if task_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "task_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    // Create analysis job
    let job = AnalysisJob::new(
        request.organization_id.clone(),
        "task".to_string(),
        task_id.clone(),
    );

    if let Err(_e) = analytics_db.create_analysis_job(&job) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Failed to create analysis job"
            })),
        );
    }

    // Spawn background task to fetch data from PostgreSQL and run analysis
    let job_id = job.id;
    spawn_task_analysis(state, job, analytics_db);

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "job_id": job_id.to_string(),
            "status": "pending",
            "message": "Analysis job created. Poll /api/analytics/jobs/{job_id} for status."
        })),
    )
}

/// Trigger analysis for a goal
/// POST /api/analytics/analysis/goal/:goal_id
pub async fn analyze_goal(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
    Json(request): Json<AnalyzeGoalRequest>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&request.organization_id) {
        return e;
    }
    if goal_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "goal_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    let job = AnalysisJob::new(
        request.organization_id.clone(),
        "goal".to_string(),
        goal_id.clone(),
    );

    if let Err(_e) = analytics_db.create_analysis_job(&job) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "Failed to create analysis job"
            })),
        );
    }

    // Spawn background task to fetch data from PostgreSQL and run analysis
    let job_id = job.id;
    spawn_goal_analysis(state, job, analytics_db);

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "job_id": job_id.to_string(),
            "status": "pending",
            "message": "Analysis job created. Poll /api/analytics/jobs/{job_id} for status."
        })),
    )
}

/// Get analysis job status
/// GET /api/analytics/jobs/:job_id
pub async fn get_analysis_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    let uuid = match Uuid::parse_str(&job_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Invalid job ID format" })),
            );
        }
    };

    match analytics_db.get_analysis_job(&uuid) {
        Ok(Some(job)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": job.id.to_string(),
                "organization_id": job.organization_id,
                "entity_type": job.entity_type,
                "entity_id": job.entity_id,
                "status": job.status.as_str(),
                "progress_percent": job.progress_percent,
                "current_stage": job.current_stage,
                "interactions_found": job.interactions_found,
                "interactions_classified": job.interactions_classified,
                "patterns_matched": job.patterns_matched,
                "recommendations_generated": job.recommendations_generated,
                "error": job.error,
                "created_at": job.created_at.to_rfc3339(),
                "updated_at": job.updated_at.to_rfc3339(),
                "completed_at": job.completed_at.map(|t| t.to_rfc3339()),
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Job not found" })),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve job" })),
        ),
    }
}

/// Get timeline for a task
/// GET /api/analytics/timeline/task/:task_id
pub async fn get_task_timeline(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    if task_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "task_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_timeline("task", &task_id) {
        Ok(Some(timeline)) => {
            match serde_json::to_value(timeline) {
                Ok(value) => (StatusCode::OK, Json(value)),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Failed to serialize timeline" })),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Timeline not found. Trigger analysis first with POST /api/analytics/analysis/task/:task_id"
            })),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve timeline" })),
        ),
    }
}

/// Get timeline for a goal
/// GET /api/analytics/timeline/goal/:goal_id
pub async fn get_goal_timeline(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> impl IntoResponse {
    if goal_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "goal_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_timeline("goal", &goal_id) {
        Ok(Some(timeline)) => {
            match serde_json::to_value(timeline) {
                Ok(value) => (StatusCode::OK, Json(value)),
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Failed to serialize timeline" })),
                ),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Timeline not found. Trigger analysis first with POST /api/analytics/analysis/goal/:goal_id"
            })),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve timeline" })),
        ),
    }
}

/// Get classified interactions for a task
/// GET /api/analytics/interactions/task/:task_id
pub async fn get_task_interactions(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    if task_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "task_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_classifications_for_task(&task_id) {
        Ok(classifications) => {
            let response: Vec<serde_json::Value> = classifications
                .into_iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id.to_string(),
                        "source_type": c.source_type.as_str(),
                        "source_id": c.source_id,
                        "sender_id": c.sender_id,
                        "content": c.content,
                        "interaction_type": c.interaction_type.as_str(),
                        "secondary_types": c.secondary_types.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
                        "confidence_score": c.confidence_score,
                        "sentiment": c.sentiment,
                        "urgency_level": c.urgency_level.as_str(),
                        "entities": c.entities,
                        "original_created_at": c.original_created_at.to_rfc3339(),
                        "classified_at": c.classified_at.to_rfc3339(),
                    })
                })
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "interactions": response })))
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve interactions" })),
        ),
    }
}

/// Search interactions by type
/// GET /api/analytics/interactions/search
pub async fn search_interactions(
    State(state): State<AppState>,
    Query(query): Query<SearchInteractionsQuery>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&query.organization_id) {
        return e;
    }

    let limit = sanitize_limit(query.limit);

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    // Parse interaction type - if not provided or invalid, search for "other" type
    // Note: A future enhancement could search all types when None is provided
    let interaction_type = query
        .interaction_type
        .as_ref()
        .map(|t| InteractionType::parse(t))
        .unwrap_or(InteractionType::Other);

    match analytics_db.search_classifications_by_type(
        &query.organization_id,
        interaction_type,
        limit,
    ) {
        Ok(classifications) => {
            let response: Vec<serde_json::Value> = classifications
                .into_iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id.to_string(),
                        "task_id": c.task_id,
                        "goal_id": c.goal_id,
                        "source_type": c.source_type.as_str(),
                        "sender_id": c.sender_id,
                        "content": c.content,
                        "interaction_type": c.interaction_type.as_str(),
                        "confidence_score": c.confidence_score,
                        "urgency_level": c.urgency_level.as_str(),
                        "original_created_at": c.original_created_at.to_rfc3339(),
                    })
                })
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "interactions": response })))
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to search interactions" })),
        ),
    }
}

/// List learned patterns
/// GET /api/analytics/patterns?organization_id=xxx
pub async fn list_patterns(
    State(state): State<AppState>,
    Query(query): Query<OrgRecommendationsQuery>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&query.organization_id) {
        return e;
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_patterns(&query.organization_id) {
        Ok(patterns) => {
            let response: Vec<serde_json::Value> = patterns
                .into_iter()
                .filter_map(|p| serde_json::to_value(p).ok())
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "patterns": response })))
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve patterns" })),
        ),
    }
}

/// Trigger pattern learning
/// POST /api/analytics/patterns/learn
pub async fn trigger_pattern_learning(
    State(_state): State<AppState>,
    Json(request): Json<PatternLearnRequest>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&request.organization_id) {
        return e;
    }

    // TODO: Implement batch pattern learning from completed tasks
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "message": "Pattern learning triggered",
            "organization_id": request.organization_id,
            "note": "This is a stub - full implementation requires completed task timelines"
        })),
    )
}

/// Get recommendations for a task
/// GET /api/analytics/recommendations/task/:task_id
pub async fn get_task_recommendations(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    if task_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "task_id is required" })),
        );
    }

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_recommendations_for_target("task", &task_id) {
        Ok(recommendations) => {
            let response: Vec<serde_json::Value> = recommendations
                .into_iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "recommendations": response })))
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve recommendations" })),
        ),
    }
}

/// Get organization-wide recommendations
/// GET /api/analytics/recommendations/organization?organization_id=xxx
pub async fn get_org_recommendations(
    State(state): State<AppState>,
    Query(query): Query<OrgRecommendationsQuery>,
) -> impl IntoResponse {
    // Validate inputs
    if let Err(e) = validate_org_id(&query.organization_id) {
        return e;
    }

    let limit = sanitize_limit(query.limit);

    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    match analytics_db.get_org_recommendations(&query.organization_id, limit) {
        Ok(recommendations) => {
            let response: Vec<serde_json::Value> = recommendations
                .into_iter()
                .filter_map(|r| serde_json::to_value(r).ok())
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "recommendations": response })))
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to retrieve recommendations" })),
        ),
    }
}

/// Submit feedback on a recommendation
/// POST /api/analytics/recommendations/:id/feedback
pub async fn submit_recommendation_feedback(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(feedback): Json<RecommendationFeedback>,
) -> impl IntoResponse {
    let analytics_db = match get_analytics_db(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };

    let uuid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Invalid recommendation ID format" })),
            );
        }
    };

    let status = RecommendationStatus::parse(&feedback.status);

    match analytics_db.update_recommendation_feedback(&uuid, status, feedback.feedback.as_deref()) {
        Ok(true) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "Feedback recorded",
                "status": feedback.status
            })),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Recommendation not found" })),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to update feedback" })),
        ),
    }
}

/// Analytics info endpoint
pub async fn analytics_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "Interaction Analytics API",
        "version": "1.0.0",
        "description": "Analyze team communications, reconstruct workflow timelines, and generate efficiency recommendations",
        "status": "active",
        "endpoints": {
            "POST /api/analytics/analysis/task/:task_id": "Trigger analysis for a task",
            "POST /api/analytics/analysis/goal/:goal_id": "Trigger analysis for a goal",
            "GET /api/analytics/jobs/:job_id": "Get analysis job status",
            "GET /api/analytics/timeline/task/:task_id": "Get task workflow timeline",
            "GET /api/analytics/timeline/goal/:goal_id": "Get goal workflow timeline",
            "GET /api/analytics/interactions/task/:task_id": "Get classified interactions for a task",
            "GET /api/analytics/interactions/search": "Search interactions by type (query params: organization_id, interaction_type, limit)",
            "GET /api/analytics/patterns": "List learned workflow patterns",
            "POST /api/analytics/patterns/learn": "Trigger pattern learning",
            "GET /api/analytics/recommendations/task/:task_id": "Get task recommendations",
            "GET /api/analytics/recommendations/organization": "Get org-wide recommendations",
            "POST /api/analytics/recommendations/:id/feedback": "Submit feedback on a recommendation"
        },
        "interaction_types": [
            "request_clarification",
            "request_resources",
            "direction",
            "suggestion",
            "request_approval",
            "status_update",
            "acknowledgment",
            "escalation",
            "blocker",
            "question",
            "answer",
            "assignment",
            "feedback",
            "recognition",
            "other"
        ]
    }))
}

// ==================== Background Job Processing ====================

/// Spawn background task analysis job
fn spawn_task_analysis(
    state: AppState,
    mut job: AnalysisJob,
    analytics_db: Arc<AnalyticsDb>,
) {
    tokio::spawn(async move {
        let task_id = job.entity_id.clone();
        let org_id = job.organization_id.clone();

        tracing::info!(
            job_id = %job.id,
            task_id = %task_id,
            org_id = %org_id,
            build = "2026-02-04-v3",
            "Starting background task analysis"
        );

        // Fetch task data from PostgreSQL
        let task_input = match fetch_task_data_from_pg(&state, &task_id, &org_id).await {
            Ok(input) => input,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "Failed to fetch task data from PostgreSQL");
                job.status = AnalysisJobStatus::Failed;
                job.error = Some(format!("Failed to fetch task data: {}", e));
                job.updated_at = Utc::now();
                let _ = analytics_db.update_analysis_job(&job);
                return;
            }
        };

        // Create processor with fast rule-based classifier (instant)
        // Use with_ollama() instead if higher-quality LLM classification is needed
        let processor = AnalyticsJobProcessor::with_rule_based(Arc::clone(&analytics_db));

        match processor.process_task_analysis(&mut job, task_input).await {
            Ok(result) => {
                tracing::info!(
                    job_id = %job.id,
                    interactions = result.classifications.len(),
                    recommendations = result.recommendations.len(),
                    "Task analysis completed"
                );
            }
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "Task analysis failed");
                job.status = AnalysisJobStatus::Failed;
                job.error = Some(format!("Analysis failed: {}", e));
                job.updated_at = Utc::now();
                let _ = analytics_db.update_analysis_job(&job);
            }
        }
    });
}

/// Spawn background goal analysis job
fn spawn_goal_analysis(
    state: AppState,
    mut job: AnalysisJob,
    analytics_db: Arc<AnalyticsDb>,
) {
    tokio::spawn(async move {
        let goal_id = job.entity_id.clone();
        let org_id = job.organization_id.clone();

        tracing::info!(
            job_id = %job.id,
            goal_id = %goal_id,
            org_id = %org_id,
            "Starting background goal analysis"
        );

        // Fetch goal's tasks from PostgreSQL and analyze them collectively
        let task_input = match fetch_goal_data_from_pg(&state, &goal_id, &org_id).await {
            Ok(input) => input,
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "Failed to fetch goal data from PostgreSQL");
                job.status = AnalysisJobStatus::Failed;
                job.error = Some(format!("Failed to fetch goal data: {}", e));
                job.updated_at = Utc::now();
                let _ = analytics_db.update_analysis_job(&job);
                return;
            }
        };

        // Create processor with fast rule-based classifier (instant)
        let processor = AnalyticsJobProcessor::with_rule_based(Arc::clone(&analytics_db));

        match processor.process_task_analysis(&mut job, task_input).await {
            Ok(result) => {
                tracing::info!(
                    job_id = %job.id,
                    interactions = result.classifications.len(),
                    recommendations = result.recommendations.len(),
                    "Goal analysis completed"
                );
            }
            Err(e) => {
                tracing::error!(job_id = %job.id, error = %e, "Goal analysis failed");
                job.status = AnalysisJobStatus::Failed;
                job.error = Some(format!("Analysis failed: {}", e));
                job.updated_at = Utc::now();
                let _ = analytics_db.update_analysis_job(&job);
            }
        }
    });
}

/// Fetch task data from PostgreSQL for analysis
///
/// Schema notes:
/// - api.tasks has no organization_id; org scoping is via team_assignments or goals
/// - api.tasks has no completed_at column
/// - api.task_comments uses author_id (not user_id) and content (not body)
/// - api.task_activity_logs uses changed_by (not user_id), changes jsonb (not details text)
/// - api.messages has no organization_id or metadata column
#[cfg(feature = "postgres")]
async fn fetch_task_data_from_pg(
    state: &AppState,
    task_id: &str,
    _org_id: &str,
) -> std::result::Result<TaskAnalysisInput, String> {
    let pool = state.pg_pool()
        .ok_or_else(|| "PostgreSQL pool not available".to_string())?;
    let client = pool.get().await
        .map_err(|e| format!("Failed to get PG connection: {}", e))?;

    // Parse task_id as UUID for parameterized queries
    let task_uuid = uuid::Uuid::parse_str(task_id)
        .map_err(|_| format!("Invalid task_id UUID: {}", task_id))?;

    // Fetch task details (no organization_id column on tasks table)
    let task_row = client
        .query_opt(
            "SELECT id, title, status, goal_id, created_at, updated_at \
             FROM api.tasks WHERE id = $1",
            &[&task_uuid],
        )
        .await
        .map_err(|e| format!("Failed to query task: {}", e))?
        .ok_or_else(|| format!("Task {} not found", task_id))?;

    let task_title: String = task_row.get("title");
    let status: String = task_row.get("status");
    let goal_id: Option<String> = task_row.get::<_, Option<uuid::Uuid>>("goal_id").map(|u| u.to_string());
    let created_at: DateTime<Utc> = task_row.get("created_at");
    // Tasks have no completed_at; use updated_at if status is done
    let completed_at: Option<DateTime<Utc>> = if status == "Done" || status == "Completed" {
        Some(task_row.get("updated_at"))
    } else {
        None
    };

    // Fetch goal title if goal_id present
    let goal_title = if let Some(ref gid) = goal_id {
        let goal_uuid = uuid::Uuid::parse_str(gid).ok();
        if let Some(gu) = goal_uuid {
            client
                .query_opt(
                    "SELECT title FROM api.goals WHERE id = $1",
                    &[&gu],
                )
                .await
                .ok()
                .flatten()
                .map(|row| row.get::<_, String>("title"))
        } else {
            None
        }
    } else {
        None
    };

    // Fetch task comments (columns: id, author_id, content, created_at)
    let comment_rows = client
        .query(
            "SELECT id, author_id, content, created_at \
             FROM api.task_comments \
             WHERE task_id = $1 \
             ORDER BY created_at ASC \
             LIMIT 500",
            &[&task_uuid],
        )
        .await
        .unwrap_or_default();

    let comments: Vec<TaskComment> = comment_rows
        .iter()
        .map(|row| TaskComment {
            id: row.get::<_, uuid::Uuid>("id").to_string(),
            author_id: row.get::<_, uuid::Uuid>("author_id").to_string(),
            content: row.get::<_, String>("content"),
            created_at: row.get("created_at"),
        })
        .collect();

    // Fetch related messages (messages table has no org_id; search by content mention)
    let search_pattern = format!("%{}%", task_id);
    let message_rows = client
        .query(
            "SELECT id, sender_id, content, created_at \
             FROM api.messages \
             WHERE content ILIKE $1 \
             ORDER BY created_at ASC \
             LIMIT 100",
            &[&search_pattern],
        )
        .await
        .unwrap_or_default();

    let related_messages: Vec<RelatedMessage> = message_rows
        .iter()
        .map(|row| RelatedMessage {
            id: row.get::<_, uuid::Uuid>("id").to_string(),
            sender_id: row.get::<_, uuid::Uuid>("sender_id").to_string(),
            content: row.get::<_, String>("content"),
            created_at: row.get("created_at"),
        })
        .collect();

    // Fetch activity logs (columns: id, action, changed_by, changed_by_name, created_at, changes)
    let activity_rows = client
        .query(
            "SELECT id, action, changed_by, changed_by_name, created_at, changes \
             FROM api.task_activity_logs \
             WHERE task_id = $1 \
             ORDER BY created_at ASC \
             LIMIT 500",
            &[&task_uuid],
        )
        .await
        .unwrap_or_default();

    let activity_events: Vec<ActivityEvent> = activity_rows
        .iter()
        .map(|row| {
            let changes_json: Option<serde_json::Value> = row.get("changes");
            ActivityEvent {
                action: row.get::<_, String>("action"),
                description: changes_json.as_ref()
                    .and_then(|v| serde_json::to_string(v).ok())
                    .unwrap_or_default(),
                actor_id: row.get::<_, Option<uuid::Uuid>>("changed_by")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                actor_name: row.get("changed_by_name"),
                timestamp: row.get("created_at"),
                changes: changes_json,
            }
        })
        .collect();

    tracing::info!(
        task_id = %task_id,
        comments = comments.len(),
        messages = related_messages.len(),
        events = activity_events.len(),
        "Fetched task data from PostgreSQL"
    );

    Ok(TaskAnalysisInput {
        task_id: task_id.to_string(),
        task_title,
        goal_id,
        goal_title,
        status,
        created_at,
        completed_at,
        comments,
        related_messages,
        activity_events,
    })
}

/// Fetch task data without PostgreSQL - returns minimal input from available context
#[cfg(not(feature = "postgres"))]
async fn fetch_task_data_from_pg(
    _state: &AppState,
    task_id: &str,
    _org_id: &str,
) -> std::result::Result<TaskAnalysisInput, String> {
    Ok(TaskAnalysisInput {
        task_id: task_id.to_string(),
        task_title: String::new(),
        goal_id: None,
        goal_title: None,
        status: "unknown".to_string(),
        created_at: Utc::now(),
        completed_at: None,
        comments: vec![],
        related_messages: vec![],
        activity_events: vec![],
    })
}

/// Fetch goal data from PostgreSQL for analysis
///
/// Schema notes:
/// - api.goals has organization_id (UUID)
/// - api.tasks has no organization_id; linked to goals via goal_id
/// - api.task_comments uses author_id and content
/// - api.task_activity_logs uses changed_by, changed_by_name, changes (jsonb)
#[cfg(feature = "postgres")]
async fn fetch_goal_data_from_pg(
    state: &AppState,
    goal_id: &str,
    org_id: &str,
) -> std::result::Result<TaskAnalysisInput, String> {
    let pool = state.pg_pool()
        .ok_or_else(|| "PostgreSQL pool not available".to_string())?;
    let client = pool.get().await
        .map_err(|e| format!("Failed to get PG connection: {}", e))?;

    let goal_uuid = uuid::Uuid::parse_str(goal_id)
        .map_err(|_| format!("Invalid goal_id UUID: {}", goal_id))?;
    let org_uuid = uuid::Uuid::parse_str(org_id)
        .map_err(|_| format!("Invalid organization_id UUID: {}", org_id))?;

    // Fetch goal details (goals table has organization_id)
    let goal_row = client
        .query_opt(
            "SELECT id, title, status, created_at \
             FROM api.goals WHERE id = $1 AND organization_id = $2",
            &[&goal_uuid, &org_uuid],
        )
        .await
        .map_err(|e| format!("Failed to query goal: {}", e))?
        .ok_or_else(|| format!("Goal {} not found in organization {}", goal_id, org_id))?;

    let goal_title: String = goal_row.get("title");
    let status: String = goal_row.get::<_, Option<String>>("status").unwrap_or_else(|| "not_started".to_string());
    let created_at: DateTime<Utc> = goal_row.get::<_, Option<DateTime<Utc>>>("created_at").unwrap_or_else(Utc::now);

    // Fetch all task comments under this goal's tasks
    let comment_rows = client
        .query(
            "SELECT tc.id, tc.author_id, tc.content, tc.created_at \
             FROM api.task_comments tc \
             JOIN api.tasks t ON tc.task_id = t.id \
             WHERE t.goal_id = $1 \
             ORDER BY tc.created_at ASC \
             LIMIT 1000",
            &[&goal_uuid],
        )
        .await
        .unwrap_or_default();

    let comments: Vec<TaskComment> = comment_rows
        .iter()
        .map(|row| TaskComment {
            id: row.get::<_, uuid::Uuid>("id").to_string(),
            author_id: row.get::<_, uuid::Uuid>("author_id").to_string(),
            content: row.get::<_, String>("content"),
            created_at: row.get("created_at"),
        })
        .collect();

    // Fetch activity logs across all tasks in this goal
    let activity_rows = client
        .query(
            "SELECT tal.id, tal.action, tal.changed_by, tal.changed_by_name, tal.created_at, tal.changes \
             FROM api.task_activity_logs tal \
             JOIN api.tasks t ON tal.task_id = t.id \
             WHERE t.goal_id = $1 \
             ORDER BY tal.created_at ASC \
             LIMIT 1000",
            &[&goal_uuid],
        )
        .await
        .unwrap_or_default();

    let activity_events: Vec<ActivityEvent> = activity_rows
        .iter()
        .map(|row| {
            let changes_json: Option<serde_json::Value> = row.get("changes");
            ActivityEvent {
                action: row.get::<_, String>("action"),
                description: changes_json.as_ref()
                    .and_then(|v| serde_json::to_string(v).ok())
                    .unwrap_or_default(),
                actor_id: row.get::<_, Option<uuid::Uuid>>("changed_by")
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                actor_name: row.get("changed_by_name"),
                timestamp: row.get("created_at"),
                changes: changes_json,
            }
        })
        .collect();

    tracing::info!(
        goal_id = %goal_id,
        comments = comments.len(),
        events = activity_events.len(),
        "Fetched goal data from PostgreSQL"
    );

    Ok(TaskAnalysisInput {
        task_id: goal_id.to_string(), // Reusing task_id field for goal
        task_title: goal_title.clone(),
        goal_id: Some(goal_id.to_string()),
        goal_title: Some(goal_title),
        status,
        created_at,
        completed_at: None,
        comments,
        related_messages: vec![],
        activity_events,
    })
}

/// Fetch goal data without PostgreSQL
#[cfg(not(feature = "postgres"))]
async fn fetch_goal_data_from_pg(
    _state: &AppState,
    goal_id: &str,
    _org_id: &str,
) -> std::result::Result<TaskAnalysisInput, String> {
    Ok(TaskAnalysisInput {
        task_id: goal_id.to_string(),
        task_title: String::new(),
        goal_id: Some(goal_id.to_string()),
        goal_title: None,
        status: "unknown".to_string(),
        created_at: Utc::now(),
        completed_at: None,
        comments: vec![],
        related_messages: vec![],
        activity_events: vec![],
    })
}

// ==================== Helpers ====================

/// Get the analytics database from app state (pre-initialized at startup)
///
/// Falls back to opening a new connection if state doesn't have one,
/// but this should rarely happen since analytics_db is initialized in AppState::new.
fn get_analytics_db(state: &AppState) -> Result<Arc<AnalyticsDb>, (StatusCode, Json<serde_json::Value>)> {
    // Prefer the pre-initialized instance from AppState (avoids re-opening on every request)
    if let Some(db) = state.analytics_db() {
        return Ok(Arc::clone(db));
    }

    // Fallback: open a new connection (should rarely happen)
    let data_dir = state.config().vector_db.storage_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let analytics_db_path = data_dir.join("analytics.db");

    match AnalyticsDb::new(&analytics_db_path) {
        Ok(db) => Ok(Arc::new(db)),
        Err(_e) => {
            tracing::error!("Failed to open analytics database");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Analytics database not available"
                })),
            ))
        }
    }
}
