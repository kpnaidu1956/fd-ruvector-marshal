//! Ollama LLM client for answer generation

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::LlmConfig;
use crate::error::{Error, Result};
use crate::types::response::Citation;

use super::prompt::PromptBuilder;

/// Ollama API client
pub struct OllamaClient {
    /// HTTP client
    client: Client,
    /// Configuration
    config: LlmConfig,
}

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Serialize)]
struct GenerateOptions {
    temperature: f32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

impl OllamaClient {
    /// Create a new Ollama client
    pub fn new(config: &LlmConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config: config.clone(),
        }
    }

    /// Check if Ollama is available
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Generate an embedding using Ollama
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.config.base_url);

        let request = EmbedRequest {
            model: self.config.embed_model.clone(),
            prompt: text.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Embedding request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Llm(format!(
                "Embedding failed: HTTP {}",
                response.status()
            )));
        }

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| Error::Llm(format!("Failed to parse embedding response: {}", e)))?;

        Ok(embed_response.embedding)
    }

    /// Generate an answer with citations
    pub async fn generate_answer(
        &self,
        question: &str,
        context: &str,
        citations: &[Citation],
    ) -> Result<String> {
        let url = format!("{}/api/generate", self.config.base_url);

        // Build prompt with citation instructions
        let prompt = PromptBuilder::build_rag_prompt(question, context, citations);

        let request = GenerateRequest {
            model: self.config.generate_model.clone(),
            prompt,
            stream: false,
            options: GenerateOptions {
                temperature: self.config.temperature,
            },
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Generation request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "Generation failed: HTTP {} - {}",
                status, body
            )));
        }

        let generate_response: GenerateResponse = response
            .json()
            .await
            .map_err(|e| Error::Llm(format!("Failed to parse generation response: {}", e)))?;

        Ok(generate_response.response)
    }

    /// Generate a streaming response (returns chunks)
    pub async fn generate_stream(
        &self,
        question: &str,
        context: &str,
        citations: &[Citation],
    ) -> Result<impl futures_util::Stream<Item = Result<String>>> {
        use futures_util::StreamExt;

        let url = format!("{}/api/generate", self.config.base_url);
        let prompt = PromptBuilder::build_rag_prompt(question, context, citations);

        #[derive(Serialize)]
        struct StreamRequest {
            model: String,
            prompt: String,
            stream: bool,
        }

        let request = StreamRequest {
            model: self.config.generate_model.clone(),
            prompt,
            stream: true,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Stream request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Llm(format!(
                "Stream failed: HTTP {}",
                response.status()
            )));
        }

        #[derive(Deserialize)]
        struct StreamChunk {
            response: String,
            done: bool,
        }

        let stream = response.bytes_stream().map(move |chunk| {
            let bytes = chunk.map_err(|e| Error::Llm(format!("Stream error: {}", e)))?;
            let text = String::from_utf8_lossy(&bytes);

            // Parse NDJSON
            let mut output = String::new();
            for line in text.lines() {
                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(line) {
                    output.push_str(&chunk.response);
                }
            }

            Ok(output)
        });

        Ok(stream)
    }
}
