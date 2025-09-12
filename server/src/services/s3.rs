//! S3/MinIO service for handling file storage
//!
//! This module provides an interface to MinIO using the AWS S3 SDK.

use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    Client,
    config::{Credentials, Region},
    primitives::ByteStream,
};
use bytes::Bytes;
use std::time::Duration;
use tracing::{debug, info};

use crate::error::{Error, Result};

/// S3 service configuration
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub region: String,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: std::env::var("MINIO_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            access_key: std::env::var("MINIO_ACCESS_KEY")
                .unwrap_or_else(|_| "slatehub".to_string()),
            secret_key: std::env::var("MINIO_SECRET_KEY")
                .unwrap_or_else(|_| "slatehub123".to_string()),
            bucket_name: std::env::var("MINIO_BUCKET")
                .unwrap_or_else(|_| "slatehub-media".to_string()),
            region: std::env::var("MINIO_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        }
    }
}

/// S3/MinIO service
pub struct S3Service {
    client: Client,
    config: S3Config,
}

impl S3Service {
    /// Create a new S3 service instance
    pub async fn new() -> Result<Self> {
        let config = S3Config::default();

        debug!("Initializing S3 service with endpoint: {}", config.endpoint);

        // Create AWS SDK credentials
        let credentials =
            Credentials::new(&config.access_key, &config.secret_key, None, None, "MinIO");

        // Configure the AWS SDK for MinIO
        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(Region::new(config.region.clone()))
            .endpoint_url(&config.endpoint)
            .force_path_style(true) // Required for MinIO
            .build();

        let client = Client::from_conf(s3_config);

        let service = Self { client, config };

        // Ensure the default bucket exists
        service.ensure_bucket_exists().await?;

        info!("S3 service initialized successfully");
        Ok(service)
    }

    /// Ensure the default bucket exists
    async fn ensure_bucket_exists(&self) -> Result<()> {
        debug!("Checking if bucket '{}' exists", self.config.bucket_name);

        match self
            .client
            .head_bucket()
            .bucket(&self.config.bucket_name)
            .send()
            .await
        {
            Ok(_) => {
                debug!("Bucket '{}' already exists", self.config.bucket_name);
                Ok(())
            }
            Err(_) => {
                info!("Creating bucket '{}'", self.config.bucket_name);
                self.client
                    .create_bucket()
                    .bucket(&self.config.bucket_name)
                    .send()
                    .await
                    .map_err(|e| Error::Internal(format!("Failed to create bucket: {}", e)))?;

                // Set bucket policy to allow public read for profile images
                // TODO: Configure proper bucket policies for different media types

                Ok(())
            }
        }
    }

    /// Upload a file to S3/MinIO
    pub async fn upload_file(&self, key: &str, data: Bytes, content_type: &str) -> Result<String> {
        debug!("Uploading file to S3: {}", key);

        let body = ByteStream::from(data);

        self.client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload file: {}", e)))?;

        info!("File uploaded successfully: {}", key);

        // Return the public URL
        Ok(format!(
            "{}/{}/{}",
            self.config.endpoint, self.config.bucket_name, key
        ))
    }

    /// Generate a presigned URL for uploading
    pub async fn generate_upload_url(&self, key: &str, content_type: &str) -> Result<String> {
        debug!("Generating presigned upload URL for: {}", key);

        let presigning_config = aws_sdk_s3::presigning::PresigningConfig::builder()
            .expires_in(Duration::from_secs(3600)) // 1 hour expiry
            .build()
            .map_err(|e| Error::Internal(format!("Failed to build presigning config: {}", e)))?;

        let presigned = self
            .client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .content_type(content_type)
            .presigned(presigning_config)
            .await
            .map_err(|e| Error::Internal(format!("Failed to generate presigned URL: {}", e)))?;

        Ok(presigned.uri().to_string())
    }

    /// Generate a presigned URL for downloading
    pub async fn generate_download_url(&self, key: &str) -> Result<String> {
        debug!("Generating presigned download URL for: {}", key);

        let presigning_config = aws_sdk_s3::presigning::PresigningConfig::builder()
            .expires_in(Duration::from_secs(86400)) // 24 hour expiry
            .build()
            .map_err(|e| Error::Internal(format!("Failed to build presigning config: {}", e)))?;

        let presigned = self
            .client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| Error::Internal(format!("Failed to generate presigned URL: {}", e)))?;

        Ok(presigned.uri().to_string())
    }

    /// Delete a file from S3/MinIO
    pub async fn delete_file(&self, key: &str) -> Result<()> {
        debug!("Deleting file from S3: {}", key);

        self.client
            .delete_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete file: {}", e)))?;

        info!("File deleted successfully: {}", key);
        Ok(())
    }

    /// Check if a file exists
    pub async fn file_exists(&self, key: &str) -> Result<bool> {
        match self
            .client
            .head_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

// Global S3 service instance
use tokio::sync::OnceCell;

static S3_SERVICE: OnceCell<S3Service> = OnceCell::const_new();

/// Initialize the global S3 service
pub async fn init_s3() -> Result<()> {
    let service = S3Service::new().await?;
    S3_SERVICE
        .set(service)
        .map_err(|_| Error::Internal("S3 service already initialized".to_string()))?;
    Ok(())
}

/// Get the global S3 service
pub fn s3() -> Result<&'static S3Service> {
    S3_SERVICE
        .get()
        .ok_or_else(|| Error::Internal("S3 service not initialized".to_string()))
}

// TODO: Future enhancements
// - Add multipart upload support for large files
// - Add file compression before upload
// - Add automatic retry logic
// - Add metrics and monitoring
// - Add support for multiple buckets
// - Add lifecycle policies for old files
// - Add encryption at rest configuration
