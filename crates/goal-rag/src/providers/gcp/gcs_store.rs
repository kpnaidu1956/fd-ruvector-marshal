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
    prefix: String,
}

impl GcsDocumentStore {
    /// Create a new GCS document store
    ///
    /// # Arguments
    /// * `auth` - GCP authentication
    /// * `bucket` - GCS bucket name
    /// * `prefix` - Object prefix (e.g., "documents/")
    pub async fn new(auth: Arc<GcpAuth>, bucket: String, prefix: Option<String>) -> Result<Self> {
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
            prefix: prefix.unwrap_or_else(|| "documents/".to_string()),
        })
    }

    /// Get the full object path for a document
    fn object_path(&self, doc_id: &Uuid, extension: &str) -> String {
        format!("{}{}.{}", self.prefix, doc_id, extension)
    }

    /// Get GCS URI for a document
    fn gcs_uri(&self, doc_id: &Uuid, extension: &str) -> String {
        format!("gs://{}/{}", self.bucket, self.object_path(doc_id, extension))
    }
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

                // Delete document
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
            prefix: Some(self.prefix.clone()),
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
