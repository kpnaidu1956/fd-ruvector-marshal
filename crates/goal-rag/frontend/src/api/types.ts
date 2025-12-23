// API Types

export interface Document {
  id: string;
  filename: string;
  file_type: string;
  total_pages: number | null;
  total_chunks: number;
  file_size: number;
  ingested_at: string;
}

export interface Citation {
  chunk_id: string;
  document_id: string;
  filename: string;
  file_type: string;
  page_number: number | null;
  section_title: string | null;
  line_start: number | null;
  line_end: number | null;
  snippet: string;
  snippet_highlighted: string;
  similarity_score: number;
  rerank_score: number | null;
}

export interface QueryRequest {
  question: string;
  top_k?: number;
  similarity_threshold?: number;
  rerank?: boolean;
  document_filter?: string[];
  include_chunks?: boolean;
}

export interface QueryResponse {
  answer: string;
  citations: Citation[];
  confidence: number;
  processing_time_ms: number;
  chunks_retrieved: number;
  chunks_used: number;
}

export interface IngestResponse {
  success: boolean;
  documents: Document[];
  total_chunks_created: number;
  processing_time_ms: number;
  errors: { filename: string; error: string }[];
}

export interface DocumentListResponse {
  documents: Document[];
  total_count: number;
}
