//! Multi-format file parser

use calamine::Reader;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::types::FileType;

/// Parsed document with extracted text and metadata
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// File type
    pub file_type: FileType,
    /// Extracted text content
    pub content: String,
    /// Content hash for deduplication
    pub content_hash: String,
    /// Total pages (if applicable)
    pub total_pages: Option<u32>,
    /// Page-level content (for PDFs, DOCX)
    pub pages: Vec<PageContent>,
    /// Document metadata
    pub metadata: HashMap<String, String>,
}

/// Content from a single page
#[derive(Debug, Clone)]
pub struct PageContent {
    /// Page number (1-indexed)
    pub page_number: u32,
    /// Text content of the page
    pub content: String,
    /// Character offset in full document
    pub char_offset: usize,
}

/// Multi-format file parser
pub struct FileParser;

impl FileParser {
    /// Parse a file based on its extension
    pub fn parse(filename: &str, data: &[u8]) -> Result<ParsedDocument> {
        let extension = filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();

        let file_type = FileType::from_extension(&extension);

        if !file_type.is_supported() {
            let reason = file_type.unsupported_reason()
                .unwrap_or("File type not supported");
            return Err(Error::UnsupportedFileType(format!("{} - {}", extension, reason)));
        }

        match file_type {
            FileType::Pdf => Self::parse_pdf(data),
            FileType::Docx | FileType::Doc => Self::parse_docx(data),
            FileType::Pptx => Self::parse_pptx(data),
            FileType::Txt | FileType::Markdown => Self::parse_text(data, file_type),
            FileType::Html => Self::parse_html(data),
            FileType::Csv => Self::parse_csv(data),
            FileType::Xlsx | FileType::Xls => Self::parse_xlsx(data),
            FileType::Code(ref lang) => Self::parse_code(data, lang.clone()),
            _ => Err(Error::UnsupportedFileType(format!("{} - File type not supported", extension))),
        }
    }

    /// Parse PDF document
    fn parse_pdf(data: &[u8]) -> Result<ParsedDocument> {
        let content = pdf_extract::extract_text_from_mem(data)
            .map_err(|e| Error::file_parse("document.pdf", e.to_string()))?;

        // For simplicity, treat entire PDF as one page
        // In production, use lopdf to get page-by-page content
        let pages = vec![PageContent {
            page_number: 1,
            content: content.clone(),
            char_offset: 0,
        }];

        // Try to count pages using lopdf
        let total_pages = match lopdf::Document::load_mem(data) {
            Ok(doc) => Some(doc.get_pages().len() as u32),
            Err(_) => Some(1),
        };

        Ok(ParsedDocument {
            file_type: FileType::Pdf,
            content_hash: hash_content(&content),
            content,
            total_pages,
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse DOCX document
    fn parse_docx(data: &[u8]) -> Result<ParsedDocument> {
        let doc = docx_rs::read_docx(data)
            .map_err(|e| Error::file_parse("document.docx", e.to_string()))?;

        let mut content = String::new();
        let mut pages = Vec::new();
        let mut current_page = String::new();
        let page_number = 1u32;

        // Extract text from document
        for child in doc.document.children {
            match child {
                docx_rs::DocumentChild::Paragraph(p) => {
                    for child in p.children {
                        if let docx_rs::ParagraphChild::Run(run) = child {
                            for child in run.children {
                                if let docx_rs::RunChild::Text(t) = child {
                                    current_page.push_str(&t.text);
                                    content.push_str(&t.text);
                                }
                            }
                        }
                    }
                    current_page.push('\n');
                    content.push('\n');
                }
                docx_rs::DocumentChild::Table(_) => {
                    // Skip tables for now
                }
                _ => {}
            }
        }

        // Treat as single page for simplicity
        if !current_page.is_empty() {
            pages.push(PageContent {
                page_number,
                content: current_page,
                char_offset: 0,
            });
        }

        Ok(ParsedDocument {
            file_type: FileType::Docx,
            content_hash: hash_content(&content),
            content,
            total_pages: Some(page_number),
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse PowerPoint presentation (.pptx)
    fn parse_pptx(data: &[u8]) -> Result<ParsedDocument> {
        use quick_xml::events::Event;
        use quick_xml::Reader;
        use std::io::Read;

        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| Error::file_parse("presentation.pptx", e.to_string()))?;

        let mut content = String::new();
        let mut pages = Vec::new();
        let mut slide_number = 0u32;

        // Find all slide files (ppt/slides/slide1.xml, slide2.xml, etc.)
        let mut slide_names: Vec<String> = archive
            .file_names()
            .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
            .map(|s| s.to_string())
            .collect();

        // Sort slides by number
        slide_names.sort_by(|a, b| {
            let num_a = a.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<u32>()
                .unwrap_or(0);
            let num_b = b.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<u32>()
                .unwrap_or(0);
            num_a.cmp(&num_b)
        });

        for slide_name in slide_names {
            slide_number += 1;
            let char_offset = content.len();

            if let Ok(mut file) = archive.by_name(&slide_name) {
                let mut xml_content = String::new();
                if file.read_to_string(&mut xml_content).is_ok() {
                    let slide_text = Self::extract_text_from_pptx_xml(&xml_content);

                    if !slide_text.is_empty() {
                        let slide_content = format!("Slide {}:\n{}\n\n", slide_number, slide_text);
                        content.push_str(&slide_content);

                        pages.push(PageContent {
                            page_number: slide_number,
                            content: slide_text,
                            char_offset,
                        });
                    }
                }
            }
        }

        // If no slides found, try to extract from other XML files
        if content.is_empty() {
            content = "Empty presentation or unable to extract text.".to_string();
        }

        let total_pages = if slide_number > 0 { Some(slide_number) } else { None };

        Ok(ParsedDocument {
            file_type: FileType::Pptx,
            content_hash: hash_content(&content),
            content,
            total_pages,
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Extract text from PowerPoint XML content
    fn extract_text_from_pptx_xml(xml: &str) -> String {
        use quick_xml::events::Event;
        use quick_xml::Reader;

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut text_parts = Vec::new();
        let mut in_text_element = false;
        let mut current_text = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    // Look for text elements: <a:t> in PPTX
                    let name = e.local_name();
                    if name.as_ref() == b"t" {
                        in_text_element = true;
                        current_text.clear();
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_text_element {
                        if let Ok(text) = e.unescape() {
                            current_text.push_str(&text);
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.local_name();
                    if name.as_ref() == b"t" && in_text_element {
                        if !current_text.trim().is_empty() {
                            text_parts.push(current_text.trim().to_string());
                        }
                        in_text_element = false;
                    }
                    // Add line break after paragraphs
                    if name.as_ref() == b"p" && !text_parts.is_empty() {
                        text_parts.push("\n".to_string());
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
        }

        // Join text parts, cleaning up extra whitespace
        text_parts
            .join(" ")
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Parse plain text or markdown
    fn parse_text(data: &[u8], file_type: FileType) -> Result<ParsedDocument> {
        let content = String::from_utf8_lossy(data).to_string();

        let pages = vec![PageContent {
            page_number: 1,
            content: content.clone(),
            char_offset: 0,
        }];

        Ok(ParsedDocument {
            file_type,
            content_hash: hash_content(&content),
            content,
            total_pages: None,
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse HTML document
    fn parse_html(data: &[u8]) -> Result<ParsedDocument> {
        let html = String::from_utf8_lossy(data);
        let document = scraper::Html::parse_document(&html);

        // Extract text from body
        let body_selector = scraper::Selector::parse("body").unwrap();
        let mut content = String::new();

        if let Some(body) = document.select(&body_selector).next() {
            for text in body.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    if !content.is_empty() {
                        content.push(' ');
                    }
                    content.push_str(trimmed);
                }
            }
        }

        let pages = vec![PageContent {
            page_number: 1,
            content: content.clone(),
            char_offset: 0,
        }];

        Ok(ParsedDocument {
            file_type: FileType::Html,
            content_hash: hash_content(&content),
            content,
            total_pages: None,
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse CSV file
    fn parse_csv(data: &[u8]) -> Result<ParsedDocument> {
        let mut reader = csv::Reader::from_reader(data);
        let mut content = String::new();

        // Get headers
        if let Ok(headers) = reader.headers() {
            content.push_str(&headers.iter().collect::<Vec<_>>().join(" | "));
            content.push('\n');
        }

        // Read rows
        for result in reader.records() {
            if let Ok(record) = result {
                content.push_str(&record.iter().collect::<Vec<_>>().join(" | "));
                content.push('\n');
            }
        }

        let pages = vec![PageContent {
            page_number: 1,
            content: content.clone(),
            char_offset: 0,
        }];

        Ok(ParsedDocument {
            file_type: FileType::Csv,
            content_hash: hash_content(&content),
            content,
            total_pages: None,
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse Excel spreadsheet
    fn parse_xlsx(data: &[u8]) -> Result<ParsedDocument> {
        let cursor = std::io::Cursor::new(data);
        let mut workbook = calamine::open_workbook_auto_from_rs(cursor)
            .map_err(|e| Error::file_parse("spreadsheet.xlsx", e.to_string()))?;

        let mut content = String::new();
        let mut pages = Vec::new();
        let mut page_number = 0u32;

        for sheet_name in workbook.sheet_names().to_vec() {
            page_number += 1;
            let char_offset = content.len();

            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut sheet_content = format!("Sheet: {}\n", sheet_name);

                for row in range.rows() {
                    let row_text: Vec<String> = row
                        .iter()
                        .map(|cell| match cell {
                            calamine::Data::Empty => String::new(),
                            calamine::Data::String(s) => s.clone(),
                            calamine::Data::Float(f) => f.to_string(),
                            calamine::Data::Int(i) => i.to_string(),
                            calamine::Data::Bool(b) => b.to_string(),
                            calamine::Data::DateTime(dt) => dt.to_string(),
                            _ => String::new(),
                        })
                        .collect();

                    if !row_text.iter().all(|s| s.is_empty()) {
                        sheet_content.push_str(&row_text.join(" | "));
                        sheet_content.push('\n');
                    }
                }

                content.push_str(&sheet_content);
                content.push('\n');

                pages.push(PageContent {
                    page_number,
                    content: sheet_content,
                    char_offset,
                });
            }
        }

        Ok(ParsedDocument {
            file_type: FileType::Xlsx,
            content_hash: hash_content(&content),
            content,
            total_pages: Some(page_number),
            pages,
            metadata: HashMap::new(),
        })
    }

    /// Parse source code file
    fn parse_code(data: &[u8], language: String) -> Result<ParsedDocument> {
        let content = String::from_utf8_lossy(data).to_string();

        let pages = vec![PageContent {
            page_number: 1,
            content: content.clone(),
            char_offset: 0,
        }];

        Ok(ParsedDocument {
            file_type: FileType::Code(language),
            content_hash: hash_content(&content),
            content,
            total_pages: None,
            pages,
            metadata: HashMap::new(),
        })
    }
}

/// Hash content for deduplication
fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
