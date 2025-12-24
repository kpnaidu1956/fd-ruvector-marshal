//! Google Cloud Storage document store
//!
//! Stores raw documents in GCS for scalable, durable storage.

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use google_cloud_storage::client::Client as GcsClient;
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::list::ListObjectsRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};

use super::auth::GcpAuth;
use crate::error::{Error, Result};
use crate::providers::document_store::{DocumentStoreProvider, StoredDocumentInfo};

/// Google Cloud Storage document store
pub struct GcsDocumentStore {
    #[allow(dead_code)]
    auth: Arc<GcpAuth>,
    client: GcsClient,
    bucket: String,
    /// Prefix for original documents
    originals_prefix: String,
    /// Prefix for extracted plain text
    plaintext_prefix: String,
}

impl GcsDocumentStore {
    /// Create a new GCS document store
    ///
    /// # Arguments
    /// * `auth` - GCP authentication
    /// * `bucket` - GCS bucket name
    /// * `originals_prefix` - Prefix for original documents (e.g., "originals/")
    /// * `plaintext_prefix` - Prefix for extracted plain text (e.g., "plaintext/")
    pub async fn new(
        auth: Arc<GcpAuth>,
        bucket: String,
        originals_prefix: Option<String>,
        plaintext_prefix: Option<String>,
    ) -> Result<Self> {
        // Create GCS client using the service account
        let config = google_cloud_storage::client::ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| Error::Config(format!("Failed to create GCS client: {}", e)))?;

        let client = GcsClient::new(config);

        Ok(Self {
            auth,
            client,
            bucket,
            originals_prefix: originals_prefix.unwrap_or_else(|| "originals/".to_string()),
            plaintext_prefix: plaintext_prefix.unwrap_or_else(|| "plaintext/".to_string()),
        })
    }

    /// Get the full object path for an original document
    fn object_path(&self, doc_id: &Uuid, extension: &str) -> String {
        format!("{}{}.{}", self.originals_prefix, doc_id, extension)
    }

    /// Get the full object path for plain text
    fn plaintext_object_path(&self, doc_id: &Uuid) -> String {
        format!("{}{}.txt", self.plaintext_prefix, doc_id)
    }

    /// Get GCS URI for an original document
    fn gcs_uri(&self, doc_id: &Uuid, extension: &str) -> String {
        format!("gs://{}/{}", self.bucket, self.object_path(doc_id, extension))
    }

    /// Get GCS URI for plain text
    fn plaintext_gcs_uri(&self, doc_id: &Uuid) -> String {
        format!("gs://{}/{}", self.bucket, self.plaintext_object_path(doc_id))
    }

    /// Store extracted plain text for a document
    ///
    /// # Arguments
    /// * `doc_id` - Document UUID
    /// * `filename` - Original filename (for metadata)
    /// * `text` - Extracted plain text content
    pub async fn store_plain_text(
        &self,
        doc_id: &Uuid,
        filename: &str,
        text: &str,
    ) -> Result<String> {
        let object_path = self.plaintext_object_path(doc_id);
        let upload_type = UploadType::Simple(Media::new(object_path.clone()));

        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                text.as_bytes().to_vec(),
                &upload_type,
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload plain text to GCS: {}", e)))?;

        tracing::debug!(
            "Stored plain text for {} ({}) at {}",
            filename,
            doc_id,
            object_path
        );

        Ok(self.plaintext_gcs_uri(doc_id))
    }

    /// Get extracted plain text for a document
    ///
    /// # Arguments
    /// * `doc_id` - Document UUID
    pub async fn get_plain_text(&self, doc_id: &Uuid) -> Result<Option<String>> {
        let object_path = self.plaintext_object_path(doc_id);

        match self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: object_path,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
        {
            Ok(data) => {
                let text = String::from_utf8(data).map_err(|e| {
                    Error::Internal(format!("Plain text is not valid UTF-8: {}", e))
                })?;
                Ok(Some(text))
            }
            Err(_) => Ok(None),
        }
    }

    /// Get document info with both original and plain text URIs
    ///
    /// # Arguments
    /// * `doc_id` - Document UUID
    pub async fn get_document_with_info(&self, doc_id: &Uuid) -> Result<Option<DocumentWithInfo>> {
        let meta_path = self.object_path(doc_id, "meta.json");

        match self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: meta_path,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
        {
            Ok(meta_data) => {
                if let Ok(metadata) = serde_json::from_slice::<DocumentMetadata>(&meta_data) {
                    let filename = metadata.filename.clone();
                    let extension = std::path::Path::new(&filename)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("bin")
                        .to_string();

                    // Check if plain text exists
                    let plaintext_uri = if self.plaintext_exists(doc_id).await {
                        Some(self.plaintext_gcs_uri(doc_id))
                    } else {
                        None
                    };

                    Ok(Some(DocumentWithInfo {
                        id: metadata.id,
                        filename,
                        size: metadata.size,
                        content_type: metadata.content_type,
                        original_uri: self.gcs_uri(doc_id, &extension),
                        plaintext_uri,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Check if plain text exists for a document
    async fn plaintext_exists(&self, doc_id: &Uuid) -> bool {
        let object_path = self.plaintext_object_path(doc_id);

        self.client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: object_path,
                ..Default::default()
            })
            .await
            .is_ok()
    }

    /// Delete plain text for a document
    pub async fn delete_plain_text(&self, doc_id: &Uuid) -> Result<()> {
        let object_path = self.plaintext_object_path(doc_id);

        let _ = self
            .client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: object_path,
                ..Default::default()
            })
            .await;

        Ok(())
    }
}

/// Document info with both original and plain text URIs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentWithInfo {
    pub id: Uuid,
    pub filename: String,
    pub size: u64,
    pub content_type: String,
    pub original_uri: String,
    pub plaintext_uri: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentMetadata {
    id: Uuid,
    filename: String,
    size: u64,
    content_type: String,
}

#[async_trait]
impl DocumentStoreProvider for GcsDocumentStore {
    async fn store_document(
        &self,
        doc_id: &Uuid,
        filename: &str,
        data: &[u8],
    ) -> Result<String> {
        // Determine content type from filename
        let content_type = mime_guess::from_path(filename)
            .first_or_octet_stream()
            .to_string();

        // Upload document data
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");

        let object_path = self.object_path(doc_id, extension);

        let upload_type = UploadType::Simple(Media::new(object_path.clone()));

        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                data.to_vec(),
                &upload_type,
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload to GCS: {}", e)))?;

        // Upload metadata
        let metadata = DocumentMetadata {
            id: *doc_id,
            filename: filename.to_string(),
            size: data.len() as u64,
            content_type,
        };
        let meta_json = serde_json::to_vec(&metadata)?;
        let meta_path = self.object_path(doc_id, "meta.json");

        let meta_upload_type = UploadType::Simple(Media::new(meta_path));

        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                meta_json,
                &meta_upload_type,
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload metadata to GCS: {}", e)))?;

        Ok(self.gcs_uri(doc_id, extension))
    }

    async fn get_document(&self, doc_id: &Uuid) -> Result<Vec<u8>> {
        // First, get metadata to find the extension
        let meta_path = self.object_path(doc_id, "meta.json");

        let meta_data = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: meta_path,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| {
                Error::Internal(format!("Failed to download metadata from GCS: {}", e))
            })?;

        let metadata: DocumentMetadata =
            serde_json::from_slice(&meta_data).map_err(|e| {
                Error::Internal(format!("Failed to parse document metadata: {}", e))
            })?;

        // Get extension from original filename
        let extension = std::path::Path::new(&metadata.filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");

        let object_path = self.object_path(doc_id, extension);

        self.client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: object_path,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to download from GCS: {}", e)))
    }

    async fn exists(&self, doc_id: &Uuid) -> Result<bool> {
        let meta_path = self.object_path(doc_id, "meta.json");

        match self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: meta_path,
                ..Default::default()
            })
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn delete_document(&self, doc_id: &Uuid) -> Result<()> {
        // Get metadata first to find the extension
        let meta_path = self.object_path(doc_id, "meta.json");

        if let Ok(meta_data) = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: meta_path.clone(),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
        {
            if let Ok(metadata) = serde_json::from_slice::<DocumentMetadata>(&meta_data) {
                let extension = std::path::Path::new(&metadata.filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("bin");

                let object_path = self.object_path(doc_id, extension);

                // Delete original document
                let _ = self
                    .client
                    .delete_object(&DeleteObjectRequest {
                        bucket: self.bucket.clone(),
                        object: object_path,
                        ..Default::default()
                    })
                    .await;
            }
        }

        // Delete plain text
        let plaintext_path = self.plaintext_object_path(doc_id);
        let _ = self
            .client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: plaintext_path,
                ..Default::default()
            })
            .await;

        // Delete metadata
        let _ = self
            .client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: meta_path,
                ..Default::default()
            })
            .await;

        Ok(())
    }

    async fn list_documents(&self) -> Result<Vec<StoredDocumentInfo>> {
        let mut docs = Vec::new();

        let list_request = ListObjectsRequest {
            bucket: self.bucket.clone(),
            prefix: Some(self.originals_prefix.clone()),
            ..Default::default()
        };

        let objects = self
            .client
            .list_objects(&list_request)
            .await
            .map_err(|e| Error::Internal(format!("Failed to list GCS objects: {}", e)))?;

        for item in objects.items.unwrap_or_default() {
            // Only process metadata files
            if item.name.ends_with(".meta.json") {
                if let Ok(meta_data) = self
                    .client
                    .download_object(
                        &GetObjectRequest {
                            bucket: self.bucket.clone(),
                            object: item.name.clone(),
                            ..Default::default()
                        },
                        &Range::default(),
                    )
                    .await
                {
                    if let Ok(metadata) = serde_json::from_slice::<DocumentMetadata>(&meta_data) {
                        let filename = metadata.filename;
                        let extension = std::path::Path::new(&filename)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("bin")
                            .to_string();

                        docs.push(StoredDocumentInfo {
                            id: metadata.id,
                            filename,
                            uri: self.gcs_uri(&metadata.id, &extension),
                            size: metadata.size,
                        });
                    }
                }
            }
        }

        Ok(docs)
    }

    async fn get_uri(&self, doc_id: &Uuid) -> Result<Option<String>> {
        let meta_path = self.object_path(doc_id, "meta.json");

        match self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: meta_path,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
        {
            Ok(meta_data) => {
                if let Ok(metadata) = serde_json::from_slice::<DocumentMetadata>(&meta_data) {
                    let extension = std::path::Path::new(&metadata.filename)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("bin");
                    Ok(Some(self.gcs_uri(doc_id, extension)))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn health_check(&self) -> Result<bool> {
        // Try to list objects (with limit 1) to check bucket access
        let list_request = ListObjectsRequest {
            bucket: self.bucket.clone(),
            max_results: Some(1),
            ..Default::default()
        };

        self.client
            .list_objects(&list_request)
            .await
            .map(|_| true)
            .map_err(|e| Error::Internal(format!("GCS health check failed: {}", e)))
    }

    fn name(&self) -> &str {
        "gcs"
    }
}
