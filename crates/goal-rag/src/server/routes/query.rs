//! Query endpoint with RAG and citations

use axum::{extract::State, Json};
use std::time::Instant;
use uuid::Uuid;

use crate::error::Result;
use crate::generation::PromptBuilder;
use crate::learning::knowledge_store::QAInteraction;
use crate::server::state::AppState;
use crate::learning::CachedCitation;
use crate::providers::vector_store::VectorSearchResult;
use crate::types::{
    query::{QueryRequest, QueryType},
    response::{CacheInfo, Citation, QueryResponse, QueryResponseV2, StringSearchResponse},
};

/// POST /api/query - Query the RAG system
pub async fn query_rag(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>> {
    let start = Instant::now();

    tracing::info!("Query: \"{}\"", request.question);

    // Detect query type - string search for short phrases, RAG for questions
    let query_type = QueryType::detect(&request.question);

    // For string search queries, use literal text matching
    if matches!(query_type, QueryType::StringSearch) {
        return string_search_query(&state, &request.question, start).await;
    }

    // Generate query embedding (using provider abstraction - Ollama or Vertex AI)
    let query_embedding = state.embedding_provider().embed(&request.question).await?;

    // Search for relevant chunks (uses Vertex AI for GCP backend)
    let mut search_results: Vec<VectorSearchResult> = state.vector_store_provider().search(
        &query_embedding,
        request.top_k * 2, // Get more for filtering
        request.document_filter.as_deref(),
    ).await?;

    // Enrich minimal chunks with full data from local store (Vertex AI workaround)
    for result in &mut search_results {
        if result.chunk.content.is_empty() || result.chunk.document_id.is_nil() {
            if let Some(full_chunk) = state.get_chunk(&result.chunk.id) {
                tracing::debug!("Enriched minimal chunk {} from local store", result.chunk.id);
                result.chunk = full_chunk;
            } else {
                tracing::warn!("Chunk {} not found in local store, using minimal data", result.chunk.id);
            }
        }
    }

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
            // Enrich with document URLs (GCS links)
            if let Some(doc) = state.get_document(&r.chunk.document_id) {
                citation.enrich_with_document(&doc);
            }
            citation
        })
        .collect();

    // Build context for LLM
    let context = PromptBuilder::build_context(&search_results);

    // Find similar past Q&A for learning
    let similar_qa = state.knowledge_store().find_similar(&request.question, 3);
    let past_qa: Vec<(String, String)> = similar_qa
        .iter()
        .filter(|qa| qa.feedback_score.unwrap_or(0) >= 0)  // Only use positive/neutral feedback
        .map(|qa| (qa.question.clone(), qa.answer.clone()))
        .collect();

    // Generate answer (using provider abstraction - Ollama or Gemini)
    let answer = if past_qa.is_empty() {
        state
            .llm_provider()
            .generate_answer(&request.question, &context, &citations)
            .await?
    } else {
        tracing::info!("Using {} learned examples for better answer", past_qa.len());
        state
            .llm_provider()
            .generate_with_learning(&request.question, &context, &citations, &past_qa)
            .await?
    };

    // Parse citations from answer and link them
    let (clean_answer, linked_citations) =
        crate::generation::citation::extract_and_link_citations(&answer, &mut citations);

    let processing_time_ms = start.elapsed().as_millis() as u64;

    let mut response = QueryResponse::new(clean_answer.clone(), linked_citations.clone(), processing_time_ms);
    response.chunks_retrieved = search_results.len();

    // Store this Q&A for learning
    let interaction = QAInteraction {
        id: Uuid::new_v4(),
        question: request.question.clone(),
        answer: clean_answer,
        citations_used: linked_citations.iter().map(|c| c.filename.clone()).collect(),
        relevance_score: search_results.first().map(|r| r.similarity).unwrap_or(0.0),
        feedback_score: None,  // Will be updated via feedback endpoint
        created_at: chrono::Utc::now(),
        document_ids: search_results.iter().map(|r| r.chunk.document_id).collect(),
    };
    let interaction_id = state.knowledge_store().store_interaction(interaction);
    response.interaction_id = Some(interaction_id);

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

/// Handle string search queries (literal text matching)
async fn string_search_query(
    state: &AppState,
    query: &str,
    start: Instant,
) -> Result<Json<QueryResponse>> {
    tracing::info!("String search: \"{}\"", query);

    // Perform literal string search (uses SQLite FTS for GCP, HNSW for local)
    let results = state.vector_store_provider().string_search(query, 10).await?;

    let processing_time_ms = start.elapsed().as_millis() as u64;

    if results.is_empty() {
        return Ok(Json(QueryResponse::not_found(processing_time_ms)));
    }

    // Build citations from string search results
    let citations: Vec<Citation> = results
        .iter()
        .map(|r| Citation {
            chunk_id: r.chunk_id,
            document_id: r.document_id,
            filename: r.filename.clone(),
            file_type: r.file_type.clone(),
            page_number: r.page_number,
            section_title: None,
            line_start: None,
            line_end: None,
            snippet: r.preview.clone(),
            snippet_highlighted: r.highlighted_snippet.clone(),
            similarity_score: 1.0, // Exact match
            rerank_score: None,
            document_url: None,
            plaintext_url: None,
        })
        .collect();

    // Build answer summarizing string search results
    let total_matches: usize = results.iter().map(|r| r.match_count).sum();
    let unique_docs: std::collections::HashSet<Uuid> = results.iter().map(|r| r.document_id).collect();

    let answer = format!(
        "Found {} occurrences of \"{}\" across {} document(s).",
        total_matches, query, unique_docs.len()
    );

    let mut response = QueryResponse::new(answer, citations, processing_time_ms);
    response.chunks_retrieved = results.len();
    response.chunks_used = results.len();

    tracing::info!(
        "String search completed in {}ms, {} matches across {} docs",
        processing_time_ms,
        total_matches,
        unique_docs.len()
    );

    Ok(Json(response))
}

/// POST /api/string-search - Direct string search endpoint
pub async fn string_search(
    State(state): State<AppState>,
    Json(request): Json<StringSearchRequest>,
) -> Result<Json<StringSearchResponse>> {
    let start = Instant::now();

    let results = state.vector_store_provider().string_search(&request.query, request.limit.unwrap_or(10)).await?;
    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok(Json(StringSearchResponse::new(request.query, results, processing_time_ms)))
}

/// Request for string search endpoint
#[derive(Debug, serde::Deserialize)]
pub struct StringSearchRequest {
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// POST /api/v2/query - V2 Query endpoint with frontend-friendly format
pub async fn query_rag_v2(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponseV2>> {
    let start = Instant::now();

    tracing::info!("V2 Query: \"{}\"", request.question);

    // Detect query type
    let query_type = QueryType::detect(&request.question);

    // For string search queries, use literal text matching
    if matches!(query_type, QueryType::StringSearch) {
        let results = state.vector_store_provider().string_search(&request.question, 10).await?;
        let processing_time_ms = start.elapsed().as_millis() as u64;

        let total_matches: usize = results.iter().map(|r| r.match_count).sum();
        let unique_docs: std::collections::HashSet<Uuid> = results.iter().map(|r| r.document_id).collect();

        let answer = if results.is_empty() {
            format!("No matches found for \"{}\".", request.question)
        } else {
            format!(
                "Found {} occurrences of \"{}\" across {} document(s).",
                total_matches, request.question, unique_docs.len()
            )
        };

        return Ok(Json(QueryResponseV2::from_string_search(
            answer,
            &results,
            processing_time_ms,
        )));
    }

    // Check cache first
    let doc_timestamps = state.get_document_timestamps();
    if let Some(cached) = state.answer_cache().get(&request.question, &doc_timestamps) {
        tracing::info!("Cache hit for query");

        // Build response from cached answer
        let citations: Vec<Citation> = cached.citations.iter().map(|c| {
            Citation {
                chunk_id: c.chunk_id,
                document_id: c.document_id,
                filename: c.filename.clone(),
                file_type: crate::types::FileType::Unknown,
                page_number: None,
                section_title: None,
                line_start: None,
                line_end: None,
                snippet: c.snippet.clone(),
                snippet_highlighted: c.snippet.clone(),
                similarity_score: c.similarity_score,
                rerank_score: None,
                document_url: None,
                plaintext_url: None,
            }
        }).collect();

        let mut response = QueryResponse::new(cached.answer.clone(), citations, start.elapsed().as_millis() as u64);
        response.chunks_retrieved = cached.citations.len();
        response.chunks_used = cached.citations.len();

        return Ok(Json(QueryResponseV2::from_response(
            &response,
            true,
            Some(CacheInfo {
                from_cache: true,
                hit_count: Some(cached.hit_count),
            }),
        )));
    }

    // Generate query embedding
    let query_embedding = state.embedding_provider().embed(&request.question).await?;

    // Search for relevant chunks (uses Vertex AI for GCP backend)
    let mut search_results: Vec<VectorSearchResult> = state.vector_store_provider().search(
        &query_embedding,
        request.top_k * 2,
        request.document_filter.as_deref(),
    ).await?;

    // Enrich minimal chunks with full data from local store (Vertex AI workaround)
    for result in &mut search_results {
        if result.chunk.content.is_empty() || result.chunk.document_id.is_nil() {
            if let Some(full_chunk) = state.get_chunk(&result.chunk.id) {
                tracing::debug!("V2: Enriched minimal chunk {} from local store", result.chunk.id);
                result.chunk = full_chunk;
            } else {
                tracing::warn!("V2: Chunk {} not found in local store, using minimal data", result.chunk.id);
            }
        }
    }

    // Filter by similarity threshold
    search_results.retain(|r| r.similarity >= request.similarity_threshold);
    search_results.truncate(request.top_k);

    if search_results.is_empty() {
        let processing_time_ms = start.elapsed().as_millis() as u64;
        let response = QueryResponse::not_found(processing_time_ms);
        return Ok(Json(QueryResponseV2::from_response(&response, true, None)));
    }

    // Create citations from search results
    let mut citations: Vec<Citation> = search_results
        .iter()
        .map(|r| {
            let mut citation = Citation::from_chunk(&r.chunk, r.similarity);
            let terms: Vec<&str> = request.question.split_whitespace().collect();
            citation.highlight_terms(&terms);
            if let Some(doc) = state.get_document(&r.chunk.document_id) {
                citation.enrich_with_document(&doc);
            }
            citation
        })
        .collect();

    // Build context for LLM
    let context = crate::generation::PromptBuilder::build_context(&search_results);

    // Generate answer
    let answer = state
        .llm_provider()
        .generate_answer(&request.question, &context, &citations)
        .await?;

    // Parse citations and link them
    let (clean_answer, linked_citations) =
        crate::generation::citation::extract_and_link_citations(&answer, &mut citations);

    let processing_time_ms = start.elapsed().as_millis() as u64;

    let mut response = QueryResponse::new(clean_answer.clone(), linked_citations.clone(), processing_time_ms);
    response.chunks_retrieved = search_results.len();

    // Cache the answer
    let cached_citations: Vec<CachedCitation> = linked_citations.iter().map(|c| {
        CachedCitation {
            chunk_id: c.chunk_id,
            document_id: c.document_id,
            filename: c.filename.clone(),
            snippet: c.snippet.clone(),
            similarity_score: c.similarity_score,
        }
    }).collect();

    state.answer_cache().put(
        &request.question,
        clean_answer,
        cached_citations,
        doc_timestamps,
    );

    // Store Q&A for learning
    let interaction = crate::learning::knowledge_store::QAInteraction {
        id: Uuid::new_v4(),
        question: request.question.clone(),
        answer: response.answer.clone(),
        citations_used: linked_citations.iter().map(|c| c.filename.clone()).collect(),
        relevance_score: search_results.first().map(|r| r.similarity).unwrap_or(0.0),
        feedback_score: None,
        created_at: chrono::Utc::now(),
        document_ids: search_results.iter().map(|r| r.chunk.document_id).collect(),
    };
    let interaction_id = state.knowledge_store().store_interaction(interaction);
    response.interaction_id = Some(interaction_id);

    tracing::info!(
        "V2 Query completed in {}ms, {} citations",
        processing_time_ms,
        response.citations.len()
    );

    Ok(Json(QueryResponseV2::from_response(
        &response,
        true,
        Some(CacheInfo {
            from_cache: false,
            hit_count: None,
        }),
    )))
}
