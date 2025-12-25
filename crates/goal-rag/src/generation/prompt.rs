//! Prompt templates for RAG generation

use crate::providers::vector_store::VectorSearchResult;
use crate::types::response::Citation;

/// Prompt builder for RAG queries
pub struct PromptBuilder;

impl PromptBuilder {
    /// Build context from search results
    pub fn build_context(results: &[VectorSearchResult]) -> String {
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

    /// Build the full RAG prompt with strict grounding
    pub fn build_rag_prompt(question: &str, context: &str, citations: &[Citation]) -> String {
        format!(
            r#"You are a document-grounded assistant that ONLY uses information from provided documents.

CRITICAL GROUNDING RULES - YOU MUST FOLLOW THESE EXACTLY:
1. ONLY use information that is EXPLICITLY stated in the CONTEXT below
2. If the answer is not in the context: respond with "This information is not available in the provided documents."
3. NEVER use external knowledge, general knowledge, or training data
4. NEVER make inferences, assumptions, or educated guesses beyond what is explicitly stated
5. Every fact, claim, or piece of information MUST have a citation in this format: [Source: filename, Page X]
6. If you're unsure whether something is in the context, it's NOT - do not include it
7. Do NOT paraphrase in ways that change meaning - stay close to the source text

RESPONSE STRUCTURE:
- Provide a clear, well-organized answer using ONLY information from the context
- Cite sources inline with each claim: [Source: filename, Page X] or [Source: filename, Lines X-Y]
- If multiple sources support a point, cite all of them
- Structure with paragraphs for readability when covering multiple aspects

CONTEXT FROM DOCUMENTS:
{context}

AVAILABLE SOURCES:
{sources}

QUESTION: {question}

Provide a grounded answer using ONLY the document content above:"#,
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

    /// Build RAG prompt with learning from past Q&A
    pub fn build_rag_prompt_with_learning(
        question: &str,
        context: &str,
        citations: &[Citation],
        past_qa: &[(String, String)],
    ) -> String {
        let past_examples = if past_qa.is_empty() {
            String::new()
        } else {
            let examples: Vec<String> = past_qa
                .iter()
                .take(3)  // Limit to 3 examples to avoid context overflow
                .map(|(q, a)| format!("Q: {}\nA: {}", q, a))
                .collect();
            format!(
                "\nHERE ARE EXAMPLES OF WELL-ANSWERED SIMILAR QUESTIONS:\n{}\n\nNow answer the new question following the same comprehensive style:\n",
                examples.join("\n\n---\n\n")
            )
        };

        format!(
            r#"You are a document-grounded assistant that ONLY uses information from provided documents.

CRITICAL GROUNDING RULES - YOU MUST FOLLOW THESE EXACTLY:
1. ONLY use information that is EXPLICITLY stated in the CONTEXT below
2. If the answer is not in the context: respond with "This information is not available in the provided documents."
3. NEVER use external knowledge, general knowledge, or training data
4. NEVER make inferences, assumptions, or educated guesses beyond what is explicitly stated
5. Every fact, claim, or piece of information MUST have a citation in this format: [Source: filename, Page X]
6. If you're unsure whether something is in the context, it's NOT - do not include it
7. Do NOT paraphrase in ways that change meaning - stay close to the source text
{past_examples}
RESPONSE STRUCTURE:
- Provide a clear, well-organized answer using ONLY information from the context
- Cite sources inline with each claim: [Source: filename, Page X] or [Source: filename, Lines X-Y]
- If multiple sources support a point, cite all of them
- Structure with paragraphs for readability when covering multiple aspects

CONTEXT FROM DOCUMENTS:
{context}

AVAILABLE SOURCES:
{sources}

QUESTION: {question}

Provide a grounded answer using ONLY the document content above:"#,
            past_examples = past_examples,
            context = context,
            sources = Self::format_sources_list(citations),
            question = question
        )
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
