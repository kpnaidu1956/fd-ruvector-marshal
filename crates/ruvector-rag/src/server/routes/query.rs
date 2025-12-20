//! Query endpoint with RAG and citations

use axum::{extract::State, Json};
use std::time::Instant;

use crate::error::Result;
use crate::generation::PromptBuilder;
use crate::server::state::AppState;
use crate::types::{query::QueryRequest, response::{Citation, QueryResponse}};

/// POST /api/query - Query the RAG system
pub async fn query_rag(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>> {
    let start = Instant::now();

    tracing::info!("Query: \"{}\"", request.question);

    // Generate query embedding using Ollama
    let query_embedding = state.ollama().embed(&request.question).await?;

    // Search for relevant chunks
    let mut search_results = state.vector_store().search(
        &query_embedding,
        request.top_k * 2, // Get more for filtering
        request.document_filter.as_deref(),
    )?;

    // Filter by similarity threshold
    search_results.retain(|r| r.similarity >= request.similarity_threshold);

    // Take top_k results
    search_results.truncate(request.top_k);

    if search_results.is_empty() {
        let processing_time_ms = start.elapsed().as_millis() as u64;
        return Ok(Json(QueryResponse::not_found(processing_time_ms)));
    }

    // Create citations from search results
    let mut citations: Vec<Citation> = search_results
        .iter()
        .map(|r| {
            let mut citation = Citation::from_chunk(&r.chunk, r.similarity);
            // Highlight query terms in snippet
            let terms: Vec<&str> = request.question.split_whitespace().collect();
            citation.highlight_terms(&terms);
            citation
        })
        .collect();

    // Build context for LLM
    let context = PromptBuilder::build_context(&search_results);

    // Generate answer using Ollama
    let answer = state
        .ollama()
        .generate_answer(&request.question, &context, &citations)
        .await?;

    // Parse citations from answer and link them
    let (clean_answer, linked_citations) =
        crate::generation::citation::extract_and_link_citations(&answer, &mut citations);

    let processing_time_ms = start.elapsed().as_millis() as u64;

    let mut response = QueryResponse::new(clean_answer, linked_citations, processing_time_ms);
    response.chunks_retrieved = search_results.len();

    // Include raw chunks if requested
    if request.include_chunks {
        response.raw_chunks = Some(search_results.into_iter().map(|r| r.chunk).collect());
    }

    tracing::info!(
        "Query completed in {}ms, {} citations",
        processing_time_ms,
        response.citations.len()
    );

    Ok(Json(response))
}
