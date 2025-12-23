//! RAG Server binary
//!
//! Run with: cargo run -p goal-rag --bin goal-rag-server

use goal_rag::{config::RagConfig, server::RagServer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "goal_rag=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!(
        r#"
╔═══════════════════════════════════════════════════════════╗
║                      Goal RAG System                      ║
║           Document Q&A with Source Citations              ║
╚═══════════════════════════════════════════════════════════╝
"#
    );

    // Load configuration
    let config = RagConfig::default();

    tracing::info!("Configuration loaded");
    tracing::info!("  - Embedding model: {}", config.embeddings.model);
    tracing::info!("  - Embedding dimensions: {}", config.embeddings.dimensions);
    tracing::info!("  - LLM model: {}", config.llm.generate_model);
    tracing::info!("  - Chunk size: {}", config.chunking.chunk_size);

    // Check Ollama
    tracing::info!("Checking Ollama at {}...", config.llm.base_url);
    let client = reqwest::Client::new();
    match client.get(format!("{}/api/tags", config.llm.base_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Ollama is running");
        }
        _ => {
            tracing::warn!("Ollama not available at {}", config.llm.base_url);
            tracing::warn!("Please start Ollama:");
            tracing::warn!("  1. Install: brew install ollama");
            tracing::warn!("  2. Start: ollama serve");
            tracing::warn!("  3. Pull models: ollama pull nomic-embed-text && ollama pull llama3.2:3b");
        }
    }

    // Create and start server
    let server = RagServer::new(config).await?;

    println!("\nServer starting...");
    println!("  API: http://{}", server.address());
    println!("  Health: http://{}/health", server.address());
    println!("  API Info: http://{}/api/info", server.address());
    println!("\nEndpoints:");
    println!("  POST /api/ingest    - Upload documents");
    println!("  POST /api/query     - Ask questions");
    println!("  GET  /api/documents - List documents");
    println!("\nPress Ctrl+C to stop\n");

    server.start().await?;

    Ok(())
}
