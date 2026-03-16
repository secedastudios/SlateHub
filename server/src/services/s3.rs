//! S3-compatible storage service for handling file uploads
//!
//! This module provides a generic S3 interface that works with any S3-compatible
//! backend (RustFS, AWS S3, etc.) using the AWS S3 SDK.

use aws_config::BehaviorVersion;
use aws_sdk_s3::{
    Client,
    config::{Credentials, Region},
    primitives::ByteStream,
};
use bytes::Bytes;
use std::time::Duration;
use tracing::{debug, info, warn};

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
            endpoint: std::env::var("S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            access_key: std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "admin".to_string()),
            secret_key: std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "password".to_string()),
            bucket_name: std::env::var("S3_BUCKET").unwrap_or_else(|_| "slatehub".to_string()),
            region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
        }
    }
}

/// Generic S3-compatible storage service
pub struct S3Service {
    client: Client,
    config: S3Config,
}

impl S3Service {
    /// Create a new S3 service instance
    pub async fn new() -> Result<Self> {
        let config = S3Config::default();

        debug!("Initializing S3 service with endpoint: {}", config.endpoint);

        let credentials =
            Credentials::new(&config.access_key, &config.secret_key, None, None, "S3");

        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(credentials)
            .region(Region::new(config.region.clone()))
            .endpoint_url(&config.endpoint)
            .force_path_style(true) // Required for S3-compatible backends
            .build();

        let client = Client::from_conf(s3_config);

        let service = Self { client, config };

        // Ensure the default bucket exists
        service.ensure_bucket_exists().await?;

        // Always apply the public-read policy for profile and organization paths
        service.set_public_read_policy().await?;

        info!("S3 service initialized successfully");
        Ok(service)
    }

    /// Ensure the default bucket exists, creating it if necessary
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
                Ok(())
            }
        }
    }

    /// Apply a public-read bucket policy for profile images and organization logos.
    ///
    /// This is called on every startup so that changes to the policy are
    /// applied even if the bucket was created by a previous run.
    async fn set_public_read_policy(&self) -> Result<()> {
        debug!(
            "Applying public-read policy for profiles/*, organizations/*, and locations/* in bucket '{}'",
            self.config.bucket_name
        );

        let policy = format!(
            r#"{{
                "Version": "2012-10-17",
                "Statement": [
                    {{
                        "Effect": "Allow",
                        "Principal": {{"AWS": ["*"]}},
                        "Action": ["s3:GetObject"],
                        "Resource": ["arn:aws:s3:::{bucket}/profiles/*"]
                    }},
                    {{
                        "Effect": "Allow",
                        "Principal": {{"AWS": ["*"]}},
                        "Action": ["s3:GetObject"],
                        "Resource": ["arn:aws:s3:::{bucket}/organizations/*"]
                    }},
                    {{
                        "Effect": "Allow",
                        "Principal": {{"AWS": ["*"]}},
                        "Action": ["s3:GetObject"],
                        "Resource": ["arn:aws:s3:::{bucket}/locations/*"]
                    }},
                    {{
                        "Effect": "Allow",
                        "Principal": {{"AWS": ["*"]}},
                        "Action": ["s3:GetObject"],
                        "Resource": ["arn:aws:s3:::{bucket}/productions/*"]
                    }}
                ]
            }}"#,
            bucket = self.config.bucket_name
        );

        match self
            .client
            .put_bucket_policy()
            .bucket(&self.config.bucket_name)
            .policy(policy)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Public-read policy applied to bucket '{}'",
                    self.config.bucket_name
                );
            }
            Err(e) => {
                // Some S3-compatible backends may not support bucket policies.
                // Log the warning but don't fail startup — object-level ACLs
                // set during upload provide a fallback.
                warn!(
                    "Could not apply bucket policy (object ACLs will be used as fallback): {}",
                    e
                );
            }
        }

        Ok(())
    }

    /// Upload a file to S3.
    ///
    /// Files under `profiles/` and `organizations/` are uploaded with a
    /// `public-read` ACL so they are directly accessible without presigned URLs.
    pub async fn upload_file(&self, key: &str, data: Bytes, content_type: &str) -> Result<String> {
        debug!("Uploading file to S3: {}", key);

        let body = ByteStream::from(data);

        let mut request = self
            .client
            .put_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .body(body)
            .content_type(content_type);

        // Profile images, organization logos, location photos, and production media are public by default
        if key.starts_with("profiles/") || key.starts_with("organizations/") || key.starts_with("locations/") || key.starts_with("productions/") {
            request = request.acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead);
        }

        request
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload file: {}", e)))?;

        info!("File uploaded successfully: {}", key);

        Ok(format!(
            "{}/{}/{}",
            self.config.endpoint, self.config.bucket_name, key
        ))
    }

    /// Generate a presigned URL for uploading (expires in 1 hour)
    pub async fn generate_upload_url(&self, key: &str, content_type: &str) -> Result<String> {
        debug!("Generating presigned upload URL for: {}", key);

        let presigning_config = aws_sdk_s3::presigning::PresigningConfig::builder()
            .expires_in(Duration::from_secs(3600))
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

    /// Generate a presigned URL for downloading (expires in 24 hours)
    pub async fn generate_download_url(&self, key: &str) -> Result<String> {
        debug!("Generating presigned download URL for: {}", key);

        let presigning_config = aws_sdk_s3::presigning::PresigningConfig::builder()
            .expires_in(Duration::from_secs(86400))
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

    /// Delete a file from S3
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

    /// List all object keys in the bucket
    pub async fn list_all_objects(&self) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(&self.config.bucket_name);

            if let Some(token) = continuation_token.take() {
                req = req.continuation_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| Error::Internal(format!("Failed to list S3 objects: {}", e)))?;

            for obj in resp.contents() {
                if let Some(key) = obj.key() {
                    keys.push(key.to_string());
                }
            }

            if resp.is_truncated() == Some(true) {
                continuation_token = resp.next_continuation_token().map(|s| s.to_string());
            } else {
                break;
            }
        }

        Ok(keys)
    }

    /// Get the bucket name
    pub fn bucket_name(&self) -> &str {
        &self.config.bucket_name
    }

    /// Check whether a file exists in S3
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

    /// Download a file from S3, returning its bytes and content-type
    pub async fn download_file(&self, key: &str) -> Result<(Bytes, String)> {
        debug!("Downloading file from S3: {}", key);

        let result = self
            .client
            .get_object()
            .bucket(&self.config.bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| Error::Internal(format!("Failed to download file: {}", e)))?;

        let content_type = result
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = result
            .body
            .collect()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read file data: {}", e)))?
            .into_bytes();

        info!(
            "File downloaded successfully: {} ({} bytes)",
            key,
            data.len()
        );
        Ok((data, content_type))
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

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
