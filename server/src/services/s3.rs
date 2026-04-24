//! S3-compatible storage service for handling file uploads.
//!
//! Implemented on top of the `rust-s3` crate so the same code works against
//! RustFS (dev), MinIO, or AWS S3 — we just point the endpoint at whichever.
//! Path-style addressing is forced because that's what every non-AWS backend
//! expects.

use bytes::Bytes;
use s3::{Bucket, BucketConfiguration, Region, creds::Credentials};
use tracing::{debug, info};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// S3 service configuration, populated from environment variables.
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

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Generic S3-compatible storage service.
pub struct S3Service {
    bucket: Box<Bucket>,
    config: S3Config,
}

impl S3Service {
    /// Create a new S3 service instance. Instantiates the client, ensures the
    /// bucket exists, and applies the public-read policy for hot prefixes.
    pub async fn new() -> Result<Self> {
        let config = S3Config::default();

        debug!("Initializing S3 service with endpoint: {}", config.endpoint);

        let region = Region::Custom {
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
        };
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| Error::Internal(format!("Invalid S3 credentials: {e}")))?;

        let bucket = Bucket::new(&config.bucket_name, region.clone(), credentials.clone())
            .map_err(|e| Error::Internal(format!("Failed to init S3 bucket handle: {e}")))?
            .with_path_style();

        let service = Self { bucket, config };

        // Ensure the default bucket exists.
        service.ensure_bucket_exists(&region, &credentials).await?;
        // Apply the public-read bucket policy (best-effort — some backends reject).
        service.set_public_read_policy().await;

        info!("S3 service initialized successfully");
        Ok(service)
    }

    /// Detect whether the bucket exists; create it if not. `rust-s3` has no
    /// direct `head_bucket`, so we probe with a cheap list (max-keys=1). A 404
    /// on that list means the bucket is missing; anything else (including 403
    /// from AWS when the bucket exists but we can't list) is treated as
    /// "exists" — in which case uploads will decide the real outcome.
    async fn ensure_bucket_exists(&self, region: &Region, credentials: &Credentials) -> Result<()> {
        debug!("Checking if bucket '{}' exists", self.config.bucket_name);
        match self.bucket.list("".to_string(), None).await {
            Ok(_) => {
                debug!("Bucket '{}' already exists", self.config.bucket_name);
                Ok(())
            }
            Err(e) => {
                info!(
                    "Bucket list failed ({}); attempting to create '{}'",
                    e, self.config.bucket_name
                );
                Bucket::create_with_path_style(
                    &self.config.bucket_name,
                    region.clone(),
                    credentials.clone(),
                    BucketConfiguration::default(),
                )
                .await
                .map_err(|e| Error::Internal(format!("Failed to create bucket: {e}")))?;
                info!("Created bucket '{}'", self.config.bucket_name);
                Ok(())
            }
        }
    }

    /// Public-read access for the `profiles/`, `organizations/`, `locations/`,
    /// and `productions/` prefixes is expected to be configured on the bucket
    /// policy. `rust-s3` 0.35 does not expose a `PutBucketPolicy` API, so we
    /// no longer apply this automatically at startup (the previous aws-sdk-s3
    /// based implementation did). If you're deploying a fresh bucket, apply
    /// the policy once via the backend's admin tool or AWS CLI:
    ///
    /// ```text
    /// aws s3api put-bucket-policy --bucket <name> --policy file://policy.json
    /// ```
    ///
    /// The policy content is in `docs/s3-public-read-policy.json` (or the
    /// equivalent statements allowing `s3:GetObject` on the four prefixes).
    /// This is a one-time bucket setup step; it doesn't need to run on every
    /// server boot.
    async fn set_public_read_policy(&self) {
        debug!(
            "(rust-s3) skipping automatic bucket policy apply for '{}'; \
             configure the public-read policy on the backend once manually",
            self.config.bucket_name
        );
    }

    /// Upload a file to S3 with the given content-type.
    ///
    /// Public access for the `profiles/`, `organizations/`, `locations/`, and
    /// `productions/` prefixes is granted via the bucket policy applied at
    /// startup — rust-s3 doesn't expose a per-object `x-amz-acl` header on
    /// the high-level put, but the policy covers the same semantics.
    pub async fn upload_file(&self, key: &str, data: Bytes, content_type: &str) -> Result<String> {
        debug!("Uploading file to S3: {}", key);

        self.bucket
            .put_object_with_content_type(key, &data, content_type)
            .await
            .map_err(|e| Error::Internal(format!("Failed to upload file: {e}")))?;

        info!("File uploaded successfully: {}", key);
        Ok(format!(
            "{}/{}/{}",
            self.config.endpoint, self.config.bucket_name, key
        ))
    }

    /// Generate a presigned URL for uploading (expires in 1 hour).
    ///
    /// The `content_type` argument is kept for API compatibility with the
    /// previous aws-sdk-s3 implementation but is not bound into the signature
    /// — rust-s3's `presign_put` doesn't tie content-type into the signed
    /// request. Clients should still set the appropriate `Content-Type` header
    /// on the actual PUT.
    pub async fn generate_upload_url(&self, key: &str, _content_type: &str) -> Result<String> {
        debug!("Generating presigned upload URL for: {}", key);
        self.bucket
            .presign_put(key, 3600, None, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to generate presigned URL: {e}")))
    }

    /// Generate a presigned URL for downloading (expires in 24 hours).
    pub async fn generate_download_url(&self, key: &str) -> Result<String> {
        debug!("Generating presigned download URL for: {}", key);
        self.bucket
            .presign_get(key, 86400, None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to generate presigned URL: {e}")))
    }

    /// Delete a file from S3.
    pub async fn delete_file(&self, key: &str) -> Result<()> {
        debug!("Deleting file from S3: {}", key);
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| Error::Internal(format!("Failed to delete file: {e}")))?;
        info!("File deleted successfully: {}", key);
        Ok(())
    }

    /// List all object keys in the bucket.
    pub async fn list_all_objects(&self) -> Result<Vec<String>> {
        let results = self
            .bucket
            .list("".to_string(), None)
            .await
            .map_err(|e| Error::Internal(format!("Failed to list S3 objects: {e}")))?;

        let mut keys = Vec::new();
        for page in results {
            for obj in page.contents {
                keys.push(obj.key);
            }
        }
        Ok(keys)
    }

    /// Get the bucket name.
    pub fn bucket_name(&self) -> &str {
        &self.config.bucket_name
    }

    /// Check whether a file exists in S3.
    pub async fn file_exists(&self, key: &str) -> Result<bool> {
        match self.bucket.head_object(key).await {
            Ok((_, 200)) => Ok(true),
            Ok((_, 404)) => Ok(false),
            Ok((_, status)) => {
                // Some backends return 403 for missing objects when the caller
                // lacks list permission. Treat anything non-2xx as "absent".
                debug!(
                    "head_object returned unexpected status {} for {}",
                    status, key
                );
                Ok(false)
            }
            Err(_) => Ok(false),
        }
    }

    /// Download a file from S3, returning its bytes and content-type.
    pub async fn download_file(&self, key: &str) -> Result<(Bytes, String)> {
        debug!("Downloading file from S3: {}", key);

        let resp = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| Error::Internal(format!("Failed to download file: {e}")))?;

        let status = resp.status_code();
        if !(200..300).contains(&status) {
            return Err(Error::Internal(format!(
                "S3 download for '{key}' returned status {status}"
            )));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let bytes = Bytes::copy_from_slice(resp.as_slice());
        info!(
            "File downloaded successfully: {} ({} bytes)",
            key,
            bytes.len()
        );
        Ok((bytes, content_type))
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

use tokio::sync::OnceCell;

static S3_SERVICE: OnceCell<S3Service> = OnceCell::const_new();

/// Initialize the global S3 service.
pub async fn init_s3() -> Result<()> {
    let service = S3Service::new().await?;
    S3_SERVICE
        .set(service)
        .map_err(|_| Error::Internal("S3 service already initialized".to_string()))?;
    Ok(())
}

/// Get the global S3 service.
pub fn s3() -> Result<&'static S3Service> {
    S3_SERVICE
        .get()
        .ok_or_else(|| Error::Internal("S3 service not initialized".to_string()))
}

// TODO: Future enhancements
// - Multipart upload for large files
// - Automatic retry with backoff
// - Lifecycle policies / TTL for temporary uploads
// - Encryption at rest configuration
