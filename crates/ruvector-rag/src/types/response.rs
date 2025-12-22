//! Response types for RAG queries

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::document::{Chunk, Document, FileType};

/// Citation from a source document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    /// Chunk ID
    pub chunk_id: Uuid,
    /// Document ID
    pub document_id: Uuid,
    /// Source filename
    pub filename: String,
    /// File type
    pub file_type: FileType,
    /// Page number (if applicable)
    pub page_number: Option<u32>,
    /// Section title (if detected)
    pub section_title: Option<String>,
    /// Line numbers for code files
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    /// Exact snippet from the source
    pub snippet: String,
    /// Snippet with highlighted query terms (<mark> tags)
    pub snippet_highlighted: String,
    /// Similarity score (0.0-1.0)
    pub similarity_score: f32,
    /// Rerank score (if reranking was enabled)
    pub rerank_score: Option<f32>,
}

impl Citation {
    /// Create a citation from a chunk and similarity score
    pub fn from_chunk(chunk: &Chunk, similarity_score: f32) -> Self {
        Self {
            chunk_id: chunk.id,
            document_id: chunk.document_id,
            filename: chunk.source.filename.clone(),
            file_type: chunk.source.file_type.clone(),
            page_number: chunk.source.page_number,
            section_title: chunk.source.section_title.clone(),
            line_start: chunk.source.line_start,
            line_end: chunk.source.line_end,
            snippet: chunk.content.clone(),
            snippet_highlighted: chunk.content.clone(),
            similarity_score,
            rerank_score: None,
        }
    }

    /// Format citation for display in text
    pub fn format_inline(&self) -> String {
        let mut parts = vec![self.filename.clone()];

        if let Some(page) = self.page_number {
            parts.push(format!("Page {}", page));
        }

        if let (Some(start), Some(end)) = (self.line_start, self.line_end) {
            parts.push(format!("Lines {}-{}", start, end));
        }

        format!("[Source: {}]", parts.join(", "))
    }

    /// Highlight query terms in the snippet
    pub fn highlight_terms(&mut self, terms: &[&str]) {
        let mut highlighted = self.snippet.clone();
        for term in terms {
            // Case-insensitive replacement with <mark> tags
            let re = regex::RegexBuilder::new(&regex::escape(term))
                .case_insensitive(true)
                .build();
            if let Ok(re) = re {
                highlighted = re
                    .replace_all(&highlighted, |caps: &regex::Captures| {
                        format!("<mark>{}</mark>", &caps[0])
                    })
                    .to_string();
            }
        }
        self.snippet_highlighted = highlighted;
    }
}

/// Response from a RAG query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    /// Generated answer in clear language
    pub answer: String,
    /// Citations with source snippets
    pub citations: Vec<Citation>,
    /// Overall confidence score (0.0-1.0)
    pub confidence: f32,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Number of chunks retrieved
    pub chunks_retrieved: usize,
    /// Number of chunks used in answer
    pub chunks_used: usize,
    /// Interaction ID for feedback/learning
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interaction_id: Option<Uuid>,
    /// Raw chunks (if include_chunks was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_chunks: Option<Vec<Chunk>>,
}

impl QueryResponse {
    /// Create a new query response
    pub fn new(answer: String, citations: Vec<Citation>, processing_time_ms: u64) -> Self {
        let confidence = if citations.is_empty() {
            0.0
        } else {
            // Average similarity score
            citations.iter().map(|c| c.similarity_score).sum::<f32>() / citations.len() as f32
        };

        Self {
            answer,
            confidence,
            chunks_retrieved: citations.len(),
            chunks_used: citations.len(),
            citations,
            processing_time_ms,
            interaction_id: None,
            raw_chunks: None,
        }
    }

    /// Create an error response when no relevant information is found
    pub fn not_found(processing_time_ms: u64) -> Self {
        Self {
            answer: "I couldn't find relevant information in the documents to answer this question.".to_string(),
            citations: Vec::new(),
            confidence: 0.0,
            processing_time_ms,
            chunks_retrieved: 0,
            chunks_used: 0,
            interaction_id: None,
            raw_chunks: None,
        }
    }
}

/// Response from document ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResponse {
    /// Whether ingestion was successful
    pub success: bool,
    /// Ingested documents
    pub documents: Vec<DocumentSummary>,
    /// Total chunks created across all documents
    pub total_chunks_created: u32,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Any errors encountered (partial success)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<IngestError>,
}

/// Summary of an ingested document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    /// Document ID
    pub id: Uuid,
    /// Filename
    pub filename: String,
    /// File type
    pub file_type: FileType,
    /// Number of pages (if applicable)
    pub total_pages: Option<u32>,
    /// Number of chunks created
    pub total_chunks: u32,
    /// File size in bytes
    pub file_size: u64,
    /// Ingestion timestamp
    pub ingested_at: chrono::DateTime<chrono::Utc>,
}

impl From<&Document> for DocumentSummary {
    fn from(doc: &Document) -> Self {
        Self {
            id: doc.id,
            filename: doc.filename.clone(),
            file_type: doc.file_type.clone(),
            total_pages: doc.total_pages,
            total_chunks: doc.total_chunks,
            file_size: doc.file_size,
            ingested_at: doc.ingested_at,
        }
    }
}

/// Error during ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestError {
    /// Filename that failed
    pub filename: String,
    /// Error message
    pub error: String,
}

/// Response for listing documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentListResponse {
    /// List of documents
    pub documents: Vec<DocumentSummary>,
    /// Total count
    pub total_count: usize,
}
