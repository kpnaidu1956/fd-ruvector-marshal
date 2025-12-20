//! Document ingestion pipeline with multi-format parsing

mod chunker;
mod parser;
mod processor;

pub use chunker::TextChunker;
pub use parser::{FileParser, ParsedDocument};
pub use processor::IngestPipeline;
