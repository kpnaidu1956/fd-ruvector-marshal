//! Core types for the RAG system

pub mod document;
pub mod query;
pub mod response;

pub use document::{Chunk, ChunkSource, Document, FileType};
pub use query::QueryRequest;
pub use response::{Citation, QueryResponse};
