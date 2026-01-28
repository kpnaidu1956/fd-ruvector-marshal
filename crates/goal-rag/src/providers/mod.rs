//! Provider abstractions for embeddings, LLM, vector storage, and document storage
//!
//! This module provides trait-based abstractions that allow switching between
//! local (Ollama) and cloud (GCP) backends.

pub mod embedding;
pub mod llm;
pub mod vector_store;
pub mod document_store;
pub mod ollama;
pub mod local;
pub mod interaction_classifier;

#[cfg(feature = "gcp")]
pub mod gcp;

pub use embedding::EmbeddingProvider;
pub use llm::LlmProvider;
pub use vector_store::VectorStoreProvider;
pub use document_store::DocumentStoreProvider;
pub use interaction_classifier::InteractionClassifier;
