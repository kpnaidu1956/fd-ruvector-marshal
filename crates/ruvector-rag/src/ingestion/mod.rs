//! Document ingestion pipeline with multi-format parsing

mod chunker;
pub mod external_parser;
mod parser;
mod processor;

pub use chunker::TextChunker;
pub use external_parser::{ExternalParser, ExternalParserConfig, ParsedExternalDocument};
pub use parser::{FileParser, ParsedDocument};
pub use processor::IngestPipeline;
