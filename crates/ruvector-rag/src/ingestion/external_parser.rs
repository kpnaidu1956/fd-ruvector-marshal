//! External document parsing via APIs (for complex/legacy formats)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{Error, Result};

/// External parser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalParserConfig {
    /// Enable external parsing for unsupported formats
    pub enabled: bool,
    /// Unstructured.io API key (optional, uses free tier if not set)
    pub unstructured_api_key: Option<String>,
    /// Unstructured.io API URL
    pub unstructured_url: String,
    /// Fallback to LibreOffice conversion
    pub use_libreoffice_fallback: bool,
}

impl Default for ExternalParserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            unstructured_api_key: None,
            unstructured_url: "https://api.unstructured.io/general/v0/general".to_string(),
            use_libreoffice_fallback: true,
        }
    }
}

/// External document parser
pub struct ExternalParser {
    client: Client,
    config: ExternalParserConfig,
}

#[derive(Debug, Deserialize)]
struct UnstructuredElement {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    element_type: String,
    text: String,
    metadata: Option<UnstructuredMetadata>,
}

#[derive(Debug, Deserialize)]
struct UnstructuredMetadata {
    page_number: Option<u32>,
    #[allow(dead_code)]
    filename: Option<String>,
}

impl ExternalParser {
    /// Create a new external parser
    pub fn new(config: ExternalParserConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Check if external parsing is available
    pub fn is_available(&self) -> bool {
        self.config.enabled
    }

    /// Parse document using Unstructured.io API
    pub async fn parse_with_unstructured(
        &self,
        filename: &str,
        data: &[u8],
    ) -> Result<ParsedExternalDocument> {
        if !self.config.enabled {
            return Err(Error::Internal("External parsing is disabled".to_string()));
        }

        let form = reqwest::multipart::Form::new()
            .part(
                "files",
                reqwest::multipart::Part::bytes(data.to_vec())
                    .file_name(filename.to_string())
            );

        let mut request = self.client
            .post(&self.config.unstructured_url)
            .multipart(form);

        // Add API key if configured
        if let Some(ref api_key) = self.config.unstructured_api_key {
            request = request.header("unstructured-api-key", api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Unstructured API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Internal(format!(
                "Unstructured API error: {} - {}",
                status, body
            )));
        }

        let elements: Vec<UnstructuredElement> = response
            .json()
            .await
            .map_err(|e| Error::Internal(format!("Failed to parse Unstructured response: {}", e)))?;

        // Combine elements into pages
        let mut pages: Vec<ExternalPage> = Vec::new();
        let mut current_page = 1u32;
        let mut current_content = String::new();

        for element in elements {
            let page_num = element.metadata
                .as_ref()
                .and_then(|m| m.page_number)
                .unwrap_or(1);

            if page_num != current_page && !current_content.is_empty() {
                pages.push(ExternalPage {
                    page_number: current_page,
                    content: std::mem::take(&mut current_content),
                });
                current_page = page_num;
            }

            if !element.text.is_empty() {
                if !current_content.is_empty() {
                    current_content.push_str("\n\n");
                }
                current_content.push_str(&element.text);
            }
        }

        // Add final page
        if !current_content.is_empty() {
            pages.push(ExternalPage {
                page_number: current_page,
                content: current_content,
            });
        }

        let full_content = pages
            .iter()
            .map(|p| p.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");

        let total_pages = pages.len() as u32;

        Ok(ParsedExternalDocument {
            content: full_content,
            pages,
            total_pages,
        })
    }

    /// Convert legacy format using LibreOffice (fallback)
    pub async fn convert_with_libreoffice(
        &self,
        filename: &str,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        use std::process::Command;
        use std::fs;
        use std::path::PathBuf;

        if !self.config.use_libreoffice_fallback {
            return Err(Error::Internal("LibreOffice fallback is disabled".to_string()));
        }

        // Create temp directory
        let temp_dir = std::env::temp_dir().join(format!("ruvector-convert-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| Error::Internal(format!("Failed to create temp dir: {}", e)))?;

        // Write input file
        let input_path = temp_dir.join(filename);
        fs::write(&input_path, data)
            .map_err(|e| Error::Internal(format!("Failed to write temp file: {}", e)))?;

        // Determine output format
        let output_ext = match filename.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
            "doc" => "docx",
            "ppt" => "pptx",
            "xls" => "xlsx",
            _ => return Err(Error::Internal("Unknown format for conversion".to_string())),
        };

        // Run LibreOffice conversion
        let output = Command::new("libreoffice")
            .args([
                "--headless",
                "--convert-to",
                output_ext,
                "--outdir",
                temp_dir.to_str().unwrap(),
                input_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| Error::Internal(format!("LibreOffice conversion failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            fs::remove_dir_all(&temp_dir).ok();
            return Err(Error::Internal(format!("LibreOffice error: {}", stderr)));
        }

        // Find and read output file
        let stem = PathBuf::from(filename)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let output_path = temp_dir.join(format!("{}.{}", stem, output_ext));

        let converted = fs::read(&output_path)
            .map_err(|e| Error::Internal(format!("Failed to read converted file: {}", e)))?;

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();

        Ok(converted)
    }

    /// Check if a file needs external parsing
    pub fn needs_external_parsing(filename: &str) -> bool {
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        matches!(ext.as_str(), "doc" | "ppt" | "xls" | "rtf" | "odt" | "odp" | "ods")
    }

    /// Check if a file needs LibreOffice conversion
    pub fn needs_conversion(filename: &str) -> bool {
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        matches!(ext.as_str(), "doc" | "ppt" | "xls")
    }
}

/// Parsed document from external service
#[derive(Debug, Clone)]
pub struct ParsedExternalDocument {
    /// Full text content
    pub content: String,
    /// Pages with content
    pub pages: Vec<ExternalPage>,
    /// Total number of pages
    pub total_pages: u32,
}

/// A page from external parsing
#[derive(Debug, Clone)]
pub struct ExternalPage {
    /// Page number (1-indexed)
    pub page_number: u32,
    /// Page content
    pub content: String,
}
