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
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::analytics::{
    AnalysisJob, InteractionType, RecommendationStatus,
};
use crate::analytics::storage::AnalyticsDb;
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
    limit.min(MAX_QUERY_LIMIT).max(1)
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

    let analytics_db = match get_analytics_db(&state).await {
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

    // TODO: Spawn background task to process analysis
    // For now, return the job ID for polling

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "job_id": job.id.to_string(),
            "status": "pending",
            "message": "Analysis job created. Poll /api/analytics/jobs/{job_id} for status.",
            "note": "Full analysis requires PostgreSQL integration (pending)"
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

    let analytics_db = match get_analytics_db(&state).await {
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

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "job_id": job.id.to_string(),
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
    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
        Ok(db) => db,
        Err(e) => return e,
    };

    // Parse interaction type - if not provided or invalid, search for "other" type
    // Note: A future enhancement could search all types when None is provided
    let interaction_type = query
        .interaction_type
        .as_ref()
        .map(|t| InteractionType::from_str(t))
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

    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
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

    let analytics_db = match get_analytics_db(&state).await {
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
    let analytics_db = match get_analytics_db(&state).await {
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

    let status = RecommendationStatus::from_str(&feedback.status);

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
        "status": "partial_implementation",
        "note": "Full functionality requires PostgreSQL integration for fetching task_comments, messages, and activity_logs",
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

// ==================== Helpers ====================

/// Get or initialize the analytics database
async fn get_analytics_db(state: &AppState) -> Result<Arc<AnalyticsDb>, (StatusCode, Json<serde_json::Value>)> {
    // Get data directory from config (same pattern as state.rs)
    let data_dir = state.config().vector_db.storage_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let analytics_db_path = data_dir.join("analytics.db");

    // Ensure directory exists
    if let Some(parent) = analytics_db_path.parent() {
        if let Err(_e) = std::fs::create_dir_all(parent) {
            tracing::error!("Failed to create analytics data directory");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to initialize analytics storage"
                })),
            ));
        }
    }

    // Open database
    match AnalyticsDb::new(&analytics_db_path) {
        Ok(db) => Ok(Arc::new(db)),
        Err(_e) => {
            tracing::error!("Failed to open analytics database");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to initialize analytics storage"
                })),
            ))
        }
    }
}
