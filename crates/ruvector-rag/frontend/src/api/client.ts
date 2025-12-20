// API Client for RAG Server

import type {
  QueryRequest,
  QueryResponse,
  IngestResponse,
  DocumentListResponse,
  Document,
} from './types';

const API_BASE = '/api';

class ApiClient {
  private async fetch<T>(
    endpoint: string,
    options?: RequestInit
  ): Promise<T> {
    const response = await fetch(`${API_BASE}${endpoint}`, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: 'Unknown error' }));
      throw new Error(error.error?.message || error.message || `HTTP ${response.status}`);
    }

    return response.json();
  }

  // Query the RAG system
  async query(request: QueryRequest): Promise<QueryResponse> {
    return this.fetch<QueryResponse>('/query', {
      method: 'POST',
      body: JSON.stringify(request),
    });
  }

  // Upload and ingest files
  async ingest(files: File[]): Promise<IngestResponse> {
    const formData = new FormData();

    for (const file of files) {
      formData.append('files', file);
    }

    const response = await fetch(`${API_BASE}/ingest`, {
      method: 'POST',
      body: formData,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({ message: 'Upload failed' }));
      throw new Error(error.error?.message || error.message || `HTTP ${response.status}`);
    }

    return response.json();
  }

  // List all documents
  async listDocuments(): Promise<DocumentListResponse> {
    return this.fetch<DocumentListResponse>('/documents');
  }

  // Get a specific document
  async getDocument(id: string): Promise<Document> {
    return this.fetch<Document>(`/documents/${id}`);
  }

  // Delete a document
  async deleteDocument(id: string): Promise<{ success: boolean; deleted_chunks: number }> {
    return this.fetch(`/documents/${id}`, {
      method: 'DELETE',
    });
  }

  // Health check
  async healthCheck(): Promise<boolean> {
    try {
      const response = await fetch('/health');
      return response.ok;
    } catch {
      return false;
    }
  }
}

export const api = new ApiClient();
