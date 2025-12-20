//! Prompt templates for RAG generation

use crate::retrieval::SearchResult;
use crate::types::response::Citation;

/// Prompt builder for RAG queries
pub struct PromptBuilder;

impl PromptBuilder {
    /// Build context from search results
    pub fn build_context(results: &[SearchResult]) -> String {
        let mut context = String::new();

        for (i, result) in results.iter().enumerate() {
            let source = &result.chunk.source;

            // Build source reference
            let source_ref = Self::format_source_ref(source, i + 1);

            context.push_str(&format!(
                "[{}] {}\n\nContent:\n{}\n\n---\n\n",
                i + 1,
                source_ref,
                result.chunk.content
            ));
        }

        context
    }

    /// Format source reference for context
    fn format_source_ref(source: &crate::types::ChunkSource, _index: usize) -> String {
        let mut parts = vec![source.filename.clone()];

        if let Some(page) = source.page_number {
            parts.push(format!("Page {}", page));
        }

        if let (Some(start), Some(end)) = (source.line_start, source.line_end) {
            parts.push(format!("Lines {}-{}", start, end));
        }

        if let Some(section) = &source.section_title {
            parts.push(format!("Section: {}", section));
        }

        parts.join(", ")
    }

    /// Build the full RAG prompt
    pub fn build_rag_prompt(question: &str, context: &str, citations: &[Citation]) -> String {
        format!(
            r#"You are a knowledgeable assistant that provides comprehensive, detailed answers based on provided documents.

IMPORTANT INSTRUCTIONS:
1. Provide a COMPREHENSIVE and DETAILED answer using information from the context below
2. Synthesize information from MULTIPLE sources when available to give a complete picture
3. Include relevant details, examples, and explanations found in the documents
4. For each claim or fact, cite the source using the format [Source: filename, Page X] or [Source: filename, Lines X-Y]
5. Structure your answer clearly with proper paragraphs when covering multiple aspects
6. If information spans multiple documents, integrate it cohesively
7. If the information is not in the context, say "I cannot find this information in the provided documents"

CONTEXT FROM DOCUMENTS:
{context}

AVAILABLE SOURCES:
{sources}

QUESTION: {question}

Provide a comprehensive, well-structured answer with citations:"#,
            context = context,
            sources = Self::format_sources_list(citations),
            question = question
        )
    }

    /// Format sources list for the prompt
    fn format_sources_list(citations: &[Citation]) -> String {
        citations
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut source = format!("[{}] {}", i + 1, c.filename);
                if let Some(page) = c.page_number {
                    source.push_str(&format!(", Page {}", page));
                }
                if let (Some(start), Some(end)) = (c.line_start, c.line_end) {
                    source.push_str(&format!(", Lines {}-{}", start, end));
                }
                source
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build a simple question-answering prompt
    pub fn build_qa_prompt(question: &str, context: &str) -> String {
        format!(
            r#"Based on the following context, answer the question. Only use information from the context.

Context:
{context}

Question: {question}

Answer:"#,
            context = context,
            question = question
        )
    }

    /// Build a summarization prompt
    pub fn build_summary_prompt(text: &str) -> String {
        format!(
            r#"Summarize the following text in clear, concise language:

{text}

Summary:"#,
            text = text
        )
    }
}
