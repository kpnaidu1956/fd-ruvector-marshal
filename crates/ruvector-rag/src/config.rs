//! Configuration for the RAG system

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::ingestion::ExternalParserConfig;

/// Main RAG system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Embedding configuration
    pub embeddings: EmbeddingConfig,
    /// Chunking configuration
    pub chunking: ChunkingConfig,
    /// Ollama/LLM configuration
    pub llm: LlmConfig,
    /// Vector database configuration
    pub vector_db: VectorDbConfig,
    /// External parser configuration
    pub external_parser: ExternalParserConfig,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            embeddings: EmbeddingConfig::default(),
            chunking: ChunkingConfig::default(),
            llm: LlmConfig::default(),
            vector_db: VectorDbConfig::default(),
            external_parser: ExternalParserConfig::default(),
        }
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Enable CORS
    pub enable_cors: bool,
    /// Maximum upload size in bytes (default: 100MB)
    pub max_upload_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            max_upload_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Model to use (default: all-MiniLM-L6-v2)
    pub model: String,
    /// Embedding dimensions (384 for MiniLM, 768 for larger models)
    pub dimensions: usize,
    /// Batch size for embedding generation
    pub batch_size: usize,
    /// Maximum sequence length
    pub max_length: usize,
    /// Cache directory for models
    pub cache_dir: PathBuf,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "nomic-embed-text".to_string(),
            dimensions: 768,
            batch_size: 32,
            max_length: 256,
            cache_dir: dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ruvector-rag")
                .join("models"),
        }
    }
}

/// Text chunking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    /// Target chunk size in characters
    pub chunk_size: usize,
    /// Overlap between chunks in characters
    pub chunk_overlap: usize,
    /// Minimum chunk size (skip smaller chunks)
    pub min_chunk_size: usize,
    /// Respect sentence boundaries
    pub respect_sentences: bool,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1024,      // Larger chunks = more context
            chunk_overlap: 200,    // More overlap = better continuity
            min_chunk_size: 100,
            respect_sentences: true,
        }
    }
}

/// LLM (Ollama) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Ollama base URL
    pub base_url: String,
    /// Embedding model name
    pub embed_model: String,
    /// Generation model name
    pub generate_model: String,
    /// Temperature for generation
    pub temperature: f32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Number of retries for failed requests
    pub max_retries: u32,
    /// Context window size (tokens)
    pub context_size: usize,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            embed_model: "nomic-embed-text".to_string(),
            generate_model: "command-r".to_string(),  // Best for RAG with citations
            temperature: 0.3,  // Lower for more factual answers
            timeout_secs: 300,  // 5 minutes for complex queries
            max_retries: 3,
            context_size: 128000,  // command-r supports 128k context
        }
    }
}

/// Vector database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDbConfig {
    /// Storage path for the vector database
    pub storage_path: PathBuf,
    /// HNSW M parameter (connections per layer)
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search parameter
    pub hnsw_ef_search: usize,
}

impl Default for VectorDbConfig {
    fn default() -> Self {
        // Use absolute path to avoid path traversal detection
        let storage_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
            .join("ruvector-rag")
            .join("vectors.db");

        Self {
            storage_path,
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
        }
    }
}
