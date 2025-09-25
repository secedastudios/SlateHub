use axum::{
    Router,
    extract::{Path, Query, multipart::Multipart},
    response::Json,
    routing::{get, post},
};
use bytes::Bytes;
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tracing::{debug, info};
use ulid::Ulid;

use crate::{db::DB, error::Error, middleware::AuthenticatedUser, services::s3::s3};

pub fn router() -> Router {
    Router::new()
        .route("/upload/profile-image", post(upload_profile_image))
        .route("/profile-image/{person_id}", get(get_profile_image_url))
        .route("/upload/organization-logo", post(upload_organization_logo))
        .route(
            "/upload/organization-logo/{org_slug}",
            post(upload_organization_logo_with_slug),
        )
        .route(
            "/organization-logo/{org_slug}",
            get(get_organization_logo_url),
        )
        .route("/debug/list-uploads", get(debug_list_uploads))
}

/// Response for successful upload
#[derive(Debug, Serialize)]
struct UploadResponse {
    media_id: String,
    url: String,
    thumbnail_url: Option<String>,
}

/// Query parameters for image processing
#[derive(Debug, Deserialize)]
struct ImageProcessParams {
    /// Crop x coordinate (0-1 range)
    crop_x: Option<f32>,
    /// Crop y coordinate (0-1 range)
    crop_y: Option<f32>,
    /// Crop zoom factor (1.0 = no zoom)
    crop_zoom: Option<f32>,
}

/// Maximum file size in bytes (10MB)
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Allowed image formats
const ALLOWED_FORMATS: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/svg+xml"];

/// Profile image dimensions
const PROFILE_IMAGE_SIZE: u32 = 400;
const THUMBNAIL_SIZE: u32 = 100;

/// Organization logo dimensions
const LOGO_SIZE: u32 = 400;
const LOGO_THUMBNAIL_SIZE: u32 = 100;

/// Upload and process a profile image
async fn upload_profile_image(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<ImageProcessParams>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, Error> {
    debug!("User {} uploading profile image", user.username);

    // Extract the image from multipart
    let mut image_data: Option<(String, String, Bytes)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::bad_request(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "image" {
            continue;
        }

        let filename = field.file_name().unwrap_or("profile.jpg").to_string();

        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        // Validate content type
        if !ALLOWED_FORMATS.contains(&content_type.as_str()) {
            return Err(Error::bad_request(format!(
                "Invalid file type: {}. Allowed types: JPEG, PNG, WebP",
                content_type
            )));
        }

        let data = field
            .bytes()
            .await
            .map_err(|e| Error::bad_request(format!("Failed to read file data: {}", e)))?;

        // Check file size
        if data.len() > MAX_FILE_SIZE {
            return Err(Error::bad_request(format!(
                "File too large. Maximum size is 10MB"
            )));
        }

        image_data = Some((filename, content_type, data));
        break;
    }

    let (_filename, _content_type, data) =
        image_data.ok_or_else(|| Error::bad_request("No image file provided"))?;

    // Process the image
    let (processed_image, thumbnail) =
        process_profile_image(&data, params.crop_x, params.crop_y, params.crop_zoom)?;

    // Generate unique keys for S3
    // Remove "person:" prefix from ID to avoid colon in S3 paths
    let sanitized_user_id = user.id.strip_prefix("person:").unwrap_or(&user.id);
    let image_id = Ulid::new().to_string();
    let main_key = format!("profiles/{}/{}.jpg", sanitized_user_id, image_id);
    let thumb_key = format!("profiles/{}/thumb_{}.jpg", sanitized_user_id, image_id);

    // Upload to S3
    let s3_service = s3()?;

    let main_url = s3_service
        .upload_file(&main_key, processed_image.clone(), "image/jpeg")
        .await?;

    let thumb_url = s3_service
        .upload_file(&thumb_key, thumbnail, "image/jpeg")
        .await?;

    // Update the person's profile with the new avatar URL
    let person_id = if user.id.starts_with("person:") {
        user.id.clone()
    } else {
        format!("person:{}", user.id)
    };

    // Update the person's profile.avatar field directly with the URL
    let update_sql = format!(
        "UPDATE {} SET profile.avatar = $avatar RETURN NONE",
        person_id
    );

    DB.query(&update_sql)
        .bind(("avatar", main_url.clone()))
        .await
        .map_err(|e| Error::Internal(format!("Failed to update profile avatar: {}", e)))?;

    info!(
        "Profile image uploaded successfully for user {}",
        user.username
    );

    Ok(Json(UploadResponse {
        media_id: image_id, // Use the generated UUID as the ID
        url: main_url.clone(),
        thumbnail_url: Some(thumb_url),
    }))
}

/// Process and crop the profile image
fn process_profile_image(
    image_data: &[u8],
    crop_x: Option<f32>,
    crop_y: Option<f32>,
    crop_zoom: Option<f32>,
) -> Result<(Bytes, Bytes), Error> {
    // Load the image
    let img = image::load_from_memory(image_data)
        .map_err(|e| Error::bad_request(format!("Invalid image file: {}", e)))?;

    // Apply crop if parameters provided
    let cropped = if let (Some(x), Some(y), Some(zoom)) = (crop_x, crop_y, crop_zoom) {
        apply_circular_crop(img, x, y, zoom)?
    } else {
        // Center crop to square
        center_crop_square(img)
    };

    // Resize for profile image
    let profile_img = cropped.resize_exact(
        PROFILE_IMAGE_SIZE,
        PROFILE_IMAGE_SIZE,
        image::imageops::FilterType::Lanczos3,
    );

    // Create thumbnail
    let thumbnail = profile_img.resize_exact(
        THUMBNAIL_SIZE,
        THUMBNAIL_SIZE,
        image::imageops::FilterType::Lanczos3,
    );

    // Convert to JPEG bytes
    let mut profile_bytes = Cursor::new(Vec::new());
    profile_img
        .write_to(&mut profile_bytes, ImageFormat::Jpeg)
        .map_err(|e| Error::Internal(format!("Failed to encode image: {}", e)))?;

    let mut thumb_bytes = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut thumb_bytes, ImageFormat::Jpeg)
        .map_err(|e| Error::Internal(format!("Failed to encode thumbnail: {}", e)))?;

    Ok((
        Bytes::from(profile_bytes.into_inner()),
        Bytes::from(thumb_bytes.into_inner()),
    ))
}

/// Apply circular crop with zoom and position
fn apply_circular_crop(
    img: DynamicImage,
    crop_x: f32,
    crop_y: f32,
    zoom: f32,
) -> Result<DynamicImage, Error> {
    let width = img.width();
    let height = img.height();

    // Calculate crop size based on zoom (smaller crop = more zoom)
    let crop_size = (width.min(height) as f32 / zoom) as u32;

    // Calculate crop position
    let max_x = width.saturating_sub(crop_size);
    let max_y = height.saturating_sub(crop_size);

    let crop_x = (crop_x.clamp(0.0, 1.0) * max_x as f32) as u32;
    let crop_y = (crop_y.clamp(0.0, 1.0) * max_y as f32) as u32;

    // Crop the image
    Ok(img.crop_imm(crop_x, crop_y, crop_size, crop_size))
}

/// Center crop image to square
fn center_crop_square(img: DynamicImage) -> DynamicImage {
    let width = img.width();
    let height = img.height();
    let size = width.min(height);

    let x = (width - size) / 2;
    let y = (height - size) / 2;

    img.crop_imm(x, y, size, size)
}

/// Get the profile image URL for a person
async fn get_profile_image_url(
    Path(person_id): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    debug!("Getting profile image for person: {}", person_id);

    // Ensure we have full record ID
    let person_record = if person_id.starts_with("person:") {
        person_id.clone()
    } else {
        format!("person:{}", person_id)
    };

    // Get the profile avatar URL directly from the person record
    let sql = format!("SELECT profile.avatar FROM {} LIMIT 1", person_record);

    let mut response = DB
        .query(&sql)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch profile avatar: {}", e)))?;

    let result: Option<serde_json::Value> = response.take(0).ok().and_then(|v| v);

    if let Some(data) = result {
        if let Some(avatar_url) = data
            .get("profile")
            .and_then(|p| p.get("avatar"))
            .and_then(|a| a.as_str())
        {
            Ok(Json(serde_json::json!({
                "url": avatar_url,
                "media_id": null,
            })))
        } else {
            // Return default avatar URL
            Ok(Json(serde_json::json!({
                "url": "/static/images/default-avatar.png",
                "media_id": null,
            })))
        }
    } else {
        // Return default avatar URL
        Ok(Json(serde_json::json!({
            "url": "/static/images/default-avatar.png",
            "media_id": null,
        })))
    }
}

/// Upload and process an organization logo
async fn upload_organization_logo(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(params): Query<ImageProcessParams>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, Error> {
    debug!("User {} uploading organization logo", user.username);

    // Extract organization slug from query params
    let mut org_slug: Option<String> = None;
    let mut image_data: Option<(String, String, Bytes)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::bad_request(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or_default().to_string();

        if name == "org_slug" {
            org_slug = Some(
                field
                    .text()
                    .await
                    .map_err(|e| Error::bad_request(format!("Failed to read org_slug: {}", e)))?,
            );
        } else if name == "image" || name == "file" {
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // Validate content type
            if !ALLOWED_FORMATS.contains(&content_type.as_str()) {
                return Err(Error::bad_request(format!(
                    "Invalid file format. Allowed: JPEG, PNG, WebP. Got: {}",
                    content_type
                )));
            }

            let filename = field.file_name().unwrap_or("upload").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| Error::bad_request(format!("Failed to read file data: {}", e)))?;

            // Check file size
            if data.len() > MAX_FILE_SIZE {
                return Err(Error::bad_request(format!(
                    "File too large. Maximum size is 10MB"
                )));
            }

            image_data = Some((filename, content_type, data));
        }
    }

    let org_slug = org_slug.ok_or_else(|| Error::bad_request("Organization slug is required"))?;
    let (filename, content_type, data) =
        image_data.ok_or_else(|| Error::bad_request("No image file provided"))?;

    debug!(
        "Processing organization logo: {} ({}, {} bytes)",
        filename,
        content_type,
        data.len()
    );

    // Check if user has permission to upload logo for this organization
    let check_sql = format!(
        "SELECT * FROM membership WHERE person = person:{} AND organization.slug = '{}' AND role IN ['owner', 'admin'] LIMIT 1",
        user.id.strip_prefix("person:").unwrap_or(&user.id),
        org_slug
    );

    let mut check_response = DB
        .query(&check_sql)
        .await
        .map_err(|e| Error::Internal(format!("Permission check failed: {}", e)))?;

    let membership: Vec<serde_json::Value> = check_response.take(0).unwrap_or_default();
    if membership.is_empty() {
        return Err(Error::Forbidden);
    }

    // Process the logo image (with optional SVG support)
    let (processed_image, thumbnail) = if content_type.contains("svg") {
        // For SVG, we'll store as-is and create a rasterized thumbnail
        let thumbnail = create_svg_thumbnail(&data)?;
        (data.clone(), thumbnail)
    } else {
        // For raster images, process normally
        process_logo_image(&data, params.crop_x, params.crop_y, params.crop_zoom)?
    };

    // Generate unique keys for S3
    let image_id = Ulid::new().to_string();
    let file_extension = if content_type.contains("svg") {
        "svg"
    } else {
        "jpg"
    };

    let main_key = format!(
        "organizations/{}/logo_{}.{}",
        org_slug, image_id, file_extension
    );
    let thumb_key = format!("organizations/{}/thumb_{}.jpg", org_slug, image_id);

    // Upload to S3
    let s3_service = s3()?;

    let main_url = s3_service
        .upload_file(&main_key, processed_image.clone(), &content_type)
        .await?;

    let thumb_url = s3_service
        .upload_file(&thumb_key, thumbnail, "image/jpeg")
        .await?;

    // Update the organization's logo field
    let update_sql = format!(
        "UPDATE organization SET logo = $logo WHERE slug = '{}'",
        org_slug
    );

    DB.query(&update_sql)
        .bind(("logo", main_url.clone()))
        .await
        .map_err(|e| Error::Internal(format!("Failed to update organization logo: {}", e)))?;

    info!("Organization logo uploaded successfully for {}", org_slug);

    Ok(Json(UploadResponse {
        media_id: image_id,
        url: main_url.clone(),
        thumbnail_url: Some(thumb_url),
    }))
}

/// Process and crop the logo image
fn process_logo_image(
    image_data: &[u8],
    crop_x: Option<f32>,
    crop_y: Option<f32>,
    crop_zoom: Option<f32>,
) -> Result<(Bytes, Bytes), Error> {
    // Load the image
    let img = image::load_from_memory(image_data)
        .map_err(|e| Error::bad_request(format!("Invalid image file: {}", e)))?;

    // Apply crop if parameters provided
    let cropped = if let (Some(x), Some(y), Some(zoom)) = (crop_x, crop_y, crop_zoom) {
        apply_circular_crop(img, x, y, zoom)?
    } else {
        // Center crop to square
        center_crop_square(img)
    };

    // Resize for logo
    let logo_img =
        cropped.resize_exact(LOGO_SIZE, LOGO_SIZE, image::imageops::FilterType::Lanczos3);

    // Create thumbnail
    let thumbnail = logo_img.resize_exact(
        LOGO_THUMBNAIL_SIZE,
        LOGO_THUMBNAIL_SIZE,
        image::imageops::FilterType::Lanczos3,
    );

    // Convert to JPEG bytes
    let mut logo_bytes = Cursor::new(Vec::new());
    logo_img
        .write_to(&mut logo_bytes, ImageFormat::Jpeg)
        .map_err(|e| Error::Internal(format!("Failed to encode image: {}", e)))?;

    let mut thumb_bytes = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut thumb_bytes, ImageFormat::Jpeg)
        .map_err(|e| Error::Internal(format!("Failed to encode thumbnail: {}", e)))?;

    Ok((
        Bytes::from(logo_bytes.into_inner()),
        Bytes::from(thumb_bytes.into_inner()),
    ))
}

/// Create a thumbnail from SVG data
fn create_svg_thumbnail(_svg_data: &[u8]) -> Result<Bytes, Error> {
    // For now, we'll just create a simple placeholder thumbnail
    // In production, you'd want to use a library like resvg to rasterize SVG
    // TODO: Implement proper SVG rasterization

    // Create a simple placeholder image
    let img = DynamicImage::new_rgb8(LOGO_THUMBNAIL_SIZE, LOGO_THUMBNAIL_SIZE);

    let mut thumb_bytes = Cursor::new(Vec::new());
    img.write_to(&mut thumb_bytes, ImageFormat::Jpeg)
        .map_err(|e| Error::Internal(format!("Failed to create thumbnail: {}", e)))?;

    Ok(Bytes::from(thumb_bytes.into_inner()))
}

/// Get the logo URL for an organization
async fn get_organization_logo_url(
    Path(org_slug): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    debug!("Getting logo for organization: {}", org_slug);

    // Query for the organization's logo URL
    let sql = format!(
        "SELECT logo FROM organization WHERE slug = '{}' LIMIT 1",
        org_slug
    );

    let mut response = DB
        .query(&sql)
        .await
        .map_err(|e| Error::Internal(format!("Failed to query organization: {}", e)))?;

    let results: Vec<serde_json::Value> = response.take(0).unwrap_or_default();

    if let Some(org) = results.first() {
        if let Some(logo_url) = org.get("logo").and_then(|l| l.as_str()) {
            return Ok(Json(serde_json::json!({
                "url": logo_url,
                "has_logo": true
            })));
        }
    }

    Ok(Json(serde_json::json!({
        "url": null,
        "has_logo": false
    })))
}

/// Upload organization logo with slug in path
async fn upload_organization_logo_with_slug(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(org_slug): Path<String>,
    Query(params): Query<ImageProcessParams>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, Error> {
    debug!(
        "User {} uploading organization logo for {}",
        user.username, org_slug
    );

    // Extract image data from multipart
    let mut image_data: Option<(String, String, Bytes)> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::bad_request(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or_default().to_string();

        if name == "image" || name == "file" {
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // Validate content type
            if !ALLOWED_FORMATS.contains(&content_type.as_str()) && !content_type.contains("svg") {
                return Err(Error::bad_request(format!(
                    "Invalid file format. Allowed: JPEG, PNG, WebP, SVG. Got: {}",
                    content_type
                )));
            }

            let filename = field.file_name().unwrap_or("upload").to_string();
            let data = field
                .bytes()
                .await
                .map_err(|e| Error::bad_request(format!("Failed to read file data: {}", e)))?;

            // Check file size
            if data.len() > MAX_FILE_SIZE {
                return Err(Error::bad_request(format!(
                    "File too large. Maximum size is 10MB"
                )));
            }

            image_data = Some((filename, content_type, data));
            break;
        }
    }

    let (filename, content_type, data) =
        image_data.ok_or_else(|| Error::bad_request("No image file provided"))?;

    debug!(
        "Processing organization logo: {} ({}, {} bytes)",
        filename,
        content_type,
        data.len()
    );

    // Check if user has permission to upload logo for this organization
    let check_sql = format!(
        "SELECT * FROM membership WHERE person = person:{} AND organization.slug = '{}' AND role IN ['owner', 'admin'] LIMIT 1",
        user.id.strip_prefix("person:").unwrap_or(&user.id),
        org_slug
    );

    let mut check_response = DB
        .query(&check_sql)
        .await
        .map_err(|e| Error::Internal(format!("Permission check failed: {}", e)))?;

    let membership: Vec<serde_json::Value> = check_response.take(0).unwrap_or_default();
    if membership.is_empty() {
        return Err(Error::Forbidden);
    }

    // Process the logo image (with optional SVG support)
    let (processed_image, thumbnail) = if content_type.contains("svg") {
        // For SVG, we'll store as-is and create a rasterized thumbnail
        let thumbnail = create_svg_thumbnail(&data)?;
        (data.clone(), thumbnail)
    } else {
        // For raster images, process normally
        process_logo_image(&data, params.crop_x, params.crop_y, params.crop_zoom)?
    };

    // Generate unique keys for S3
    let image_id = Ulid::new().to_string();
    let file_extension = if content_type.contains("svg") {
        "svg"
    } else {
        "jpg"
    };

    let main_key = format!(
        "organizations/{}/logo_{}.{}",
        org_slug, image_id, file_extension
    );
    let thumb_key = format!("organizations/{}/thumb_{}.jpg", org_slug, image_id);

    // Upload to S3
    let s3_service = s3()?;

    let main_url = s3_service
        .upload_file(&main_key, processed_image.clone(), &content_type)
        .await?;

    let thumb_url = s3_service
        .upload_file(&thumb_key, thumbnail, "image/jpeg")
        .await?;

    // Update the organization's logo field
    let update_sql = format!(
        "UPDATE organization SET logo = $logo WHERE slug = '{}'",
        org_slug
    );

    DB.query(&update_sql)
        .bind(("logo", main_url.clone()))
        .await
        .map_err(|e| Error::Internal(format!("Failed to update organization logo: {}", e)))?;

    info!("Organization logo uploaded successfully for {}", org_slug);

    Ok(Json(UploadResponse {
        media_id: image_id,
        url: main_url.clone(),
        thumbnail_url: Some(thumb_url),
    }))
}

/// Debug endpoint to list uploaded files
async fn debug_list_uploads() -> Result<Json<serde_json::Value>, Error> {
    debug!("Listing uploaded files in MinIO");

    // Check if files exist in MinIO
    let s3_service = s3()?;

    // List files in the profiles directory
    let test_keys = vec!["profiles/"];

    let mut found_files = Vec::new();

    for prefix in test_keys {
        // Check if we can generate a URL for this prefix
        match s3_service.generate_download_url(prefix).await {
            Ok(url) => {
                found_files.push(serde_json::json!({
                    "prefix": prefix,
                    "url": url,
                    "status": "accessible"
                }));
            }
            Err(e) => {
                found_files.push(serde_json::json!({
                    "prefix": prefix,
                    "error": e.to_string(),
                    "status": "error"
                }));
            }
        }
    }

    // Also check the database for media records
    let media_check_sql = "SELECT id, filename, object_key, url FROM media LIMIT 10";
    let mut response = crate::db::DB.query(media_check_sql).await?;

    // Try to get records without deserializing to specific type
    let media_records: Vec<serde_json::Value> = response.take(0).unwrap_or_default();

    Ok(Json(serde_json::json!({
        "minio_files": found_files,
        "database_records": media_records,
        "message": "Debug info for uploaded files"
    })))
}

// TODO: Future enhancements
// - Add image quality settings
// - Support for multiple aspect ratios
// - Add image filters/effects
// - Implement face detection for better auto-cropping
// - Add support for animated images (GIF, WebP)
// - Implement image optimization (WebP conversion, etc.)
// - Add batch upload support
// - Add drag-and-drop reordering for multiple images
// - Implement progressive image loading
// - Add image CDN integration
