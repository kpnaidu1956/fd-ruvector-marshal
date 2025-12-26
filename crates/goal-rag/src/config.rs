//! Configuration for the RAG system

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::ingestion::ExternalParserConfig;

/// Main RAG system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Backend provider (local or gcp)
    #[serde(default)]
    pub backend: BackendProvider,
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
    /// Processing configuration
    pub processing: ProcessingConfig,
    /// GCP configuration (required when backend = gcp)
    #[serde(default)]
    pub gcp: Option<GcpConfig>,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            backend: BackendProvider::default(),
            server: ServerConfig::default(),
            embeddings: EmbeddingConfig::default(),
            chunking: ChunkingConfig::default(),
            llm: LlmConfig::default(),
            vector_db: VectorDbConfig::default(),
            external_parser: ExternalParserConfig::default(),
            processing: ProcessingConfig::default(),
            gcp: None,
        }
    }
}

/// Processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Timeout for processing a single file in seconds (default: 300 = 5 minutes)
    pub file_timeout_secs: u64,
    /// Number of parallel file workers
    pub parallel_files: Option<usize>,
    /// Number of parallel embeddings per file
    pub parallel_embeddings: Option<usize>,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            file_timeout_secs: 300, // 5 minutes
            parallel_files: None,   // Auto-detect from CPU count
            parallel_embeddings: None,
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
            generate_model: "phi3".to_string(),  // Fast 3.8B model for CPU
            temperature: 0.3,  // Lower for more factual answers
            timeout_secs: 120,  // 2 minutes for phi3
            max_retries: 2,
            context_size: 4096,  // phi3 context size
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

/// Backend provider selection
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackendProvider {
    /// Local backend (Ollama + HNSW + filesystem)
    #[default]
    Local,
    /// Google Cloud Platform (Vertex AI + GCS)
    Gcp,
}

/// Google Cloud Platform configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpConfig {
    /// Path to service account JSON key file
    pub service_account_key_path: PathBuf,
    /// GCP project ID
    pub project_id: String,
    /// GCP region (e.g., "us-central1")
    pub location: String,
    /// GCS bucket for document storage
    pub gcs_bucket: String,
    /// GCS prefix for original documents (default: "originals/")
    #[serde(default = "default_gcs_originals_prefix")]
    pub gcs_originals_prefix: String,
    /// GCS prefix for extracted plain text (default: "plaintext/")
    #[serde(default = "default_gcs_plaintext_prefix")]
    pub gcs_plaintext_prefix: String,
    /// Vertex AI Vector Search Index (full resource name for upsert operations)
    /// e.g., "projects/my-project/locations/us-central1/indexes/123456"
    pub vector_search_index: String,
    /// Vertex AI Vector Search endpoint (full resource name for query operations)
    /// e.g., "projects/my-project/locations/us-central1/indexEndpoints/123456"
    pub vector_search_endpoint: String,
    /// Public endpoint domain for Vector Search queries (required for public endpoints)
    /// e.g., "399775135.us-central1-1040167267396.vdb.vertexai.goog"
    pub vector_search_public_domain: Option<String>,
    /// Deployed index ID within the endpoint
    pub deployed_index_id: String,
    /// Embedding model (default: "text-embedding-005")
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// Generation model (default: "gemini-2.5-pro")
    #[serde(default = "default_generation_model")]
    pub generation_model: String,
    /// Document AI processor ID for PDF extraction (optional)
    /// e.g., "projects/my-project/locations/us/processors/abc123"
    /// If not set, Document AI fallback is disabled
    #[serde(default)]
    pub document_ai_processor: Option<String>,
    /// Enable Document AI as fallback for failed PDF parsing (default: true if processor is set)
    #[serde(default = "default_use_document_ai")]
    pub use_document_ai_fallback: bool,
}

fn default_embedding_model() -> String {
    "text-embedding-005".to_string()
}

fn default_generation_model() -> String {
    "gemini-2.5-pro".to_string()
}

fn default_gcs_originals_prefix() -> String {
    "originals/".to_string()
}

fn default_gcs_plaintext_prefix() -> String {
    "plaintext/".to_string()
}

fn default_use_document_ai() -> bool {
    true
}
