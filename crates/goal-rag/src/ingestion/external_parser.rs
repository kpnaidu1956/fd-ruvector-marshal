//! External document parsing via APIs and local tools (for complex/legacy formats)
//!
//! Supports:
//! - pdftotext (poppler-utils) - Fast, reliable PDF text extraction
//! - pandoc - Universal document converter
//! - LibreOffice - Legacy format conversion
//! - Unstructured.io API - Cloud-based parsing fallback

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Command;
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
    /// Use local tools (pdftotext, pandoc) first before API
    pub prefer_local_tools: bool,
}

impl Default for ExternalParserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            unstructured_api_key: None,
            unstructured_url: "https://api.unstructured.io/general/v0/general".to_string(),
            use_libreoffice_fallback: true,
            prefer_local_tools: true, // Use local tools by default
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

    /// Check if pdftotext is available
    pub fn has_pdftotext() -> bool {
        Command::new("pdftotext")
            .arg("-v")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if pandoc is available
    pub fn has_pandoc() -> bool {
        Command::new("pandoc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Convert PDF to text using pdftotext (poppler-utils)
    /// Much faster and more reliable than Rust PDF libraries for complex fonts
    pub fn convert_pdf_with_pdftotext(&self, data: &[u8]) -> Result<String> {
        use std::io::Write;
        use std::process::Stdio;

        // Try stdin/stdout first (faster, no temp files)
        let mut child = Command::new("pdftotext")
            .args([
                "-layout",      // Maintain original layout
                "-nopgbrk",     // Don't insert page breaks
                "-enc", "UTF-8", // Output encoding
                "-",            // Read from stdin
                "-",            // Write to stdout
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Internal(format!("Failed to spawn pdftotext: {}", e)))?;

        // Write PDF data to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(data)
                .map_err(|e| Error::Internal(format!("Failed to write to pdftotext: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Internal(format!("pdftotext failed: {}", e)))?;

        if !output.status.success() {
            // Fallback: use temp files (some pdftotext versions don't support stdin)
            return self.convert_pdf_with_pdftotext_tempfile(data);
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();

        if text.trim().is_empty() {
            return Err(Error::Internal("pdftotext produced no output".to_string()));
        }

        Ok(text)
    }

    /// Fallback: Convert PDF using temp files
    fn convert_pdf_with_pdftotext_tempfile(&self, data: &[u8]) -> Result<String> {
        use std::fs;

        let temp_dir = std::env::temp_dir().join(format!("goal-rag-pdf-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| Error::Internal(format!("Failed to create temp dir: {}", e)))?;

        let input_path = temp_dir.join("input.pdf");
        let output_path = temp_dir.join("output.txt");

        fs::write(&input_path, data)
            .map_err(|e| Error::Internal(format!("Failed to write temp PDF: {}", e)))?;

        let output = Command::new("pdftotext")
            .args([
                "-layout",
                "-nopgbrk",
                "-enc", "UTF-8",
                input_path.to_str().unwrap(),
                output_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| Error::Internal(format!("pdftotext failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            fs::remove_dir_all(&temp_dir).ok();
            return Err(Error::Internal(format!("pdftotext error: {}", stderr)));
        }

        let text = fs::read_to_string(&output_path)
            .map_err(|e| Error::Internal(format!("Failed to read pdftotext output: {}", e)))?;

        fs::remove_dir_all(&temp_dir).ok();

        if text.trim().is_empty() {
            return Err(Error::Internal("pdftotext produced no output".to_string()));
        }

        Ok(text)
    }

    /// Convert document to text using pandoc
    /// Supports: docx, doc, pptx, odt, rtf, epub, html, and many more
    pub fn convert_with_pandoc(&self, filename: &str, data: &[u8]) -> Result<String> {
        use std::fs;

        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

        // Pandoc input formats
        let input_format = match ext.as_str() {
            "docx" => "docx",
            "doc" => "doc",
            "odt" => "odt",
            "rtf" => "rtf",
            "epub" => "epub",
            "html" | "htm" => "html",
            "md" | "markdown" => "markdown",
            "tex" => "latex",
            "rst" => "rst",
            "pptx" => "pptx",
            _ => return Err(Error::Internal(format!("Pandoc doesn't support .{}", ext))),
        };

        let temp_dir = std::env::temp_dir().join(format!("goal-rag-pandoc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| Error::Internal(format!("Failed to create temp dir: {}", e)))?;

        let input_path = temp_dir.join(filename);
        fs::write(&input_path, data)
            .map_err(|e| Error::Internal(format!("Failed to write temp file: {}", e)))?;

        let output = Command::new("pandoc")
            .args([
                "-f", input_format,
                "-t", "plain",           // Output plain text
                "--wrap=none",           // Don't wrap lines
                input_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| Error::Internal(format!("pandoc failed: {}", e)))?;

        fs::remove_dir_all(&temp_dir).ok();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Internal(format!("pandoc error: {}", stderr)));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();

        if text.trim().is_empty() {
            return Err(Error::Internal("pandoc produced no output".to_string()));
        }

        Ok(text)
    }

    /// Smart conversion: try local tools first, then fall back to API/libraries
    pub async fn convert_to_text(&self, filename: &str, data: &[u8]) -> Result<String> {
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

        // For PDFs, try pdftotext first
        if ext == "pdf" {
            if Self::has_pdftotext() {
                tracing::info!("[{}] Using pdftotext for PDF conversion", filename);
                match self.convert_pdf_with_pdftotext(data) {
                    Ok(text) => {
                        tracing::info!("[{}] pdftotext extracted {} chars", filename, text.len());
                        return Ok(text);
                    }
                    Err(e) => {
                        tracing::warn!("[{}] pdftotext failed: {}, will try other methods", filename, e);
                    }
                }
            }
        }

        // For other formats, try pandoc
        if Self::has_pandoc() && matches!(ext.as_str(), "docx" | "doc" | "odt" | "rtf" | "pptx" | "epub") {
            tracing::info!("[{}] Using pandoc for document conversion", filename);
            match self.convert_with_pandoc(filename, data) {
                Ok(text) => {
                    tracing::info!("[{}] pandoc extracted {} chars", filename, text.len());
                    return Ok(text);
                }
                Err(e) => {
                    tracing::warn!("[{}] pandoc failed: {}, will try other methods", filename, e);
                }
            }
        }

        // Fall back to Unstructured API
        tracing::info!("[{}] Falling back to Unstructured API", filename);
        let parsed = self.parse_with_unstructured(filename, data).await?;
        Ok(parsed.content)
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
