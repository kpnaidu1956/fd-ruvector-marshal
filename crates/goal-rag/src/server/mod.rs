//! HTTP server for the RAG system

pub mod routes;
pub mod state;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::config::RagConfig;
use crate::error::Result;
use state::AppState;

/// RAG HTTP Server
pub struct RagServer {
    config: RagConfig,
    state: AppState,
}

impl RagServer {
    /// Create a new RAG server
    pub async fn new(config: RagConfig) -> Result<Self> {
        let state = AppState::new(config.clone()).await?;
        Ok(Self { config, state })
    }

    /// Create with default configuration
    pub async fn default() -> Result<Self> {
        Self::new(RagConfig::default()).await
    }

    /// Build the router with all routes
    fn build_router(&self) -> Router {
        // CORS layer - must be added first (outermost)
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        

        Router::new()
            // Health check
            .route("/health", get(health_check))
            .route("/ready", get(readiness))
            // API routes with body limit for multipart uploads
            .nest("/api", routes::api_routes(self.config.server.max_upload_size))
            .with_state(self.state.clone())
            // Middleware layers (order matters - applied bottom to top)
            .layer(TraceLayer::new_for_http())
            .layer(CompressionLayer::new())
            .layer(cors)
    }

    /// Start the server
    pub async fn start(self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.server.host, self.config.server.port)
            .parse()
            .map_err(|e| crate::error::Error::Config(format!("Invalid address: {}", e)))?;

        let router = self.build_router();

        tracing::info!("Starting RAG server on http://{}", addr);
        tracing::info!("API documentation: http://{}/api", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| crate::error::Error::Config(format!("Failed to bind: {}", e)))?;

        axum::serve(listener, router)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Get the server address
    pub fn address(&self) -> String {
        format!("{}:{}", self.config.server.host, self.config.server.port)
    }
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Readiness check endpoint
async fn readiness(state: axum::extract::State<AppState>) -> axum::http::StatusCode {
    if state.is_ready() {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    }
}
