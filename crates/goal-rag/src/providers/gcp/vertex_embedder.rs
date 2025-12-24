//! Vertex AI embedding provider using text-embedding-005
//!
//! Provides fast, high-quality embeddings with 768 dimensions.

use async_trait::async_trait;
use std::sync::Arc;

use super::auth::GcpAuth;
use crate::error::{Error, Result};
use crate::providers::embedding::EmbeddingProvider;

/// Vertex AI embedding provider
pub struct VertexAiEmbedder {
    auth: Arc<GcpAuth>,
    model: String,
    location: String,
    dimensions: usize,
}

impl VertexAiEmbedder {
    /// Create a new Vertex AI embedder
    ///
    /// # Arguments
    /// * `auth` - GCP authentication
    /// * `location` - GCP region (e.g., "us-central1")
    /// * `model` - Model name (default: "text-embedding-005")
    pub fn new(auth: Arc<GcpAuth>, location: String, model: Option<String>) -> Self {
        Self {
            auth,
            model: model.unwrap_or_else(|| "text-embedding-005".to_string()),
            location,
            dimensions: 768, // text-embedding-005 produces 768-dim vectors
        }
    }

    /// Get the API endpoint URL
    fn endpoint(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:predict",
            self.location,
            self.auth.project_id(),
            self.location,
            self.model
        )
    }
}

#[derive(serde::Serialize)]
struct EmbedRequest {
    instances: Vec<EmbedInstance>,
}

#[derive(serde::Serialize)]
struct EmbedInstance {
    content: String,
}

#[derive(serde::Deserialize)]
struct EmbedResponse {
    predictions: Vec<EmbedPrediction>,
}

#[derive(serde::Deserialize)]
struct EmbedPrediction {
    embeddings: EmbeddingValues,
}

#[derive(serde::Deserialize)]
struct EmbeddingValues {
    values: Vec<f32>,
}

#[async_trait]
impl EmbeddingProvider for VertexAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let client = self.auth.authorized_client().await?;

        let request = EmbedRequest {
            instances: vec![EmbedInstance {
                content: text.to_string(),
            }],
        };

        let response = client
            .post(&self.endpoint())
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Embedding(format!("Vertex AI request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Embedding(format!(
                "Vertex AI embedding failed ({}): {}",
                status, body
            )));
        }

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| Error::Embedding(format!("Failed to parse Vertex AI response: {}", e)))?;

        embed_response
            .predictions
            .into_iter()
            .next()
            .map(|p| p.embeddings.values)
            .ok_or_else(|| Error::Embedding("No embedding in response".to_string()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let client = self.auth.authorized_client().await?;

        // Vertex AI supports batching up to 250 texts per request
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(250) {
            let request = EmbedRequest {
                instances: chunk
                    .iter()
                    .map(|t| EmbedInstance {
                        content: t.clone(),
                    })
                    .collect(),
            };

            let response = client
                .post(&self.endpoint())
                .json(&request)
                .send()
                .await
                .map_err(|e| Error::Embedding(format!("Vertex AI batch request failed: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(Error::Embedding(format!(
                    "Vertex AI batch embedding failed ({}): {}",
                    status, body
                )));
            }

            let embed_response: EmbedResponse = response.json().await.map_err(|e| {
                Error::Embedding(format!("Failed to parse Vertex AI batch response: {}", e))
            })?;

            all_embeddings.extend(
                embed_response
                    .predictions
                    .into_iter()
                    .map(|p| p.embeddings.values),
            );
        }

        Ok(all_embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn health_check(&self) -> Result<bool> {
        // Try to get a token - if auth works, we're healthy
        self.auth.get_token().await.map(|_| true)
    }

    fn name(&self) -> &str {
        "vertex-ai"
    }
}
