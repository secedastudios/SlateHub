# Media Proxy Documentation

## Overview

The media proxy feature allows the SlateHub application to serve media files (profile images, organization logos, etc.) through the application server instead of exposing MinIO/S3 URLs directly to clients. This provides better security, control, and consistency for media delivery.

## Why Use a Media Proxy?

### Previous Architecture (Direct MinIO Access)
- Media files were served directly from MinIO URLs (e.g., `http://localhost:9000/slatehub-media/profiles/...`)
- MinIO service had to be publicly accessible
- URLs exposed internal infrastructure details
- Limited control over caching, access control, and monitoring

### New Architecture (Media Proxy)
- All media served through the application at `/api/media/*`
- MinIO remains internal and not exposed to the internet
- Centralized control over media delivery
- Better security and access control
- Consistent URL structure
- Ability to add caching, transformation, and monitoring

## How It Works

### Upload Flow
1. User uploads an image through the application
2. Application processes the image (resize, crop, etc.)
3. Processed image is uploaded to MinIO/S3
4. Application stores the proxy URL (`/api/media/path/to/file`) in the database instead of the MinIO URL
5. Application returns the proxy URL to the client

### Retrieval Flow
1. Client requests media from `/api/media/path/to/file`
2. Application receives the request
3. Application fetches the file from MinIO using the path
4. Application returns the file with appropriate headers (content-type, caching, etc.)
5. Client receives and displays the media

## Configuration

### Environment Variables

```bash
# MinIO/S3 Configuration
MINIO_ENDPOINT=http://localhost:9000      # MinIO server endpoint
MINIO_ACCESS_KEY=slatehub                 # MinIO access key
MINIO_SECRET_KEY=slatehub123              # MinIO secret key
MINIO_BUCKET=slatehub-media              # Bucket name for media storage
MINIO_REGION=us-east-1                   # AWS region (required for S3 SDK)
```

### MinIO Setup

1. Ensure MinIO is running and accessible from the application server
2. The bucket will be automatically created if it doesn't exist
3. MinIO does NOT need to be publicly accessible anymore

## API Endpoints

### Media Proxy Endpoint
```
GET /api/media/*path
```
Serves media files from MinIO through the application proxy.

**Example:**
```
GET /api/media/profiles/user123/image.jpg
```

**Response Headers:**
- `Content-Type`: Appropriate MIME type (e.g., `image/jpeg`)
- `Cache-Control`: `public, max-age=31536000` (1 year cache)

### Upload Endpoints

#### Profile Image Upload
```
POST /api/media/upload/profile-image
```
Upload a new profile image for the authenticated user.

**Returns:**
```json
{
  "media_id": "unique-id",
  "url": "/api/media/profiles/user/image.jpg",
  "thumbnail_url": "/api/media/profiles/user/thumb_image.jpg"
}
```

#### Organization Logo Upload
```
POST /api/media/upload/organization-logo/{org_slug}
```
Upload a logo for an organization (requires admin/owner permissions).

**Returns:**
```json
{
  "media_id": "unique-id",
  "url": "/api/media/organizations/org-slug/logo.jpg",
  "thumbnail_url": "/api/media/organizations/org-slug/thumb_logo.jpg"
}
```

## Migration Guide

### Migrating Existing Media URLs

If you have existing data with direct MinIO URLs, use the provided migration script:

1. Install dependencies:
```bash
npm install surrealdb.js
```

2. Run the migration in dry-run mode first:
```bash
node migrate_media_urls.js --dry-run
```

3. Review the output to ensure URLs will be migrated correctly

4. Run the actual migration:
```bash
node migrate_media_urls.js
```

The script will:
- Find all person records with profile avatars
- Find all organization records with logos
- Convert MinIO URLs to proxy URLs
- Update the database records

### Manual URL Conversion

If you need to manually convert URLs:

**Before (MinIO URL):**
```
http://localhost:9000/slatehub-media/profiles/user123/image.jpg
```

**After (Proxy URL):**
```
/api/media/profiles/user123/image.jpg
```

## Implementation Details

### Rust Code Structure

#### S3 Service (`services/s3.rs`)
- `download_file()` - New method to fetch files from MinIO
- Returns file bytes and content-type

#### Media Routes (`routes/media.rs`)
- `proxy_media()` - New handler for serving media through proxy
- Modified upload handlers to return proxy URLs instead of MinIO URLs

### Key Features

1. **Automatic Content-Type Detection**
   - Content-type is preserved from MinIO
   - Falls back to `application/octet-stream` if not available

2. **Caching Headers**
   - Media files are served with long cache headers (1 year)
   - Reduces server load and improves performance

3. **Error Handling**
   - 404 errors for missing files
   - 500 errors for MinIO connectivity issues
   - Detailed error logging for debugging

## Security Considerations

### Access Control
- Currently, all media files are publicly accessible through the proxy
- Future enhancements can add authentication checks based on file paths
- Example: Private media could be stored under `/private/` prefix

### Rate Limiting
- Consider implementing rate limiting on the proxy endpoint
- Prevents abuse and excessive bandwidth usage

### File Validation
- Upload endpoints validate file types and sizes
- Maximum file size: 10MB (configurable)
- Allowed formats: JPEG, PNG, WebP, GIF, SVG (for logos)

## Performance Optimization

### Current Implementation
- Files are fetched from MinIO on each request
- Suitable for small to medium traffic

### Future Enhancements

1. **In-Memory Caching**
   - Cache frequently accessed files in memory
   - Use LRU cache with size limits

2. **CDN Integration**
   - Serve media through a CDN
   - Use proxy URLs as origin

3. **Image Transformation**
   - On-the-fly image resizing
   - Format conversion (WebP for modern browsers)

4. **Streaming Large Files**
   - Implement streaming for video/large files
   - Reduces memory usage

## Monitoring and Logging

### Current Logging
- Each proxy request is logged with debug level
- Upload operations logged with info level
- Errors logged with error level

### Metrics to Track
- Number of proxy requests
- Cache hit/miss ratio (when implemented)
- Average response time
- Bandwidth usage
- Most requested files

## Troubleshooting

### Common Issues

1. **404 Not Found**
   - File doesn't exist in MinIO
   - Check the path is correct
   - Verify file was uploaded successfully

2. **500 Internal Server Error**
   - MinIO connectivity issues
   - Check MinIO is running
   - Verify credentials are correct

3. **Wrong Content-Type**
   - File uploaded with incorrect content-type
   - Re-upload the file with correct type

### Debug Endpoints

```
GET /api/media/debug/list-uploads
```
Lists recent uploads for debugging (development only).

## Example HTML Usage

### Profile Image
```html
<img src="/api/media/profiles/user123/image.jpg" 
     alt="User Profile" 
     class="avatar" />
```

### Organization Logo
```html
<img src="/api/media/organizations/acme-corp/logo.svg" 
     alt="ACME Corp Logo" 
     class="logo" />
```

### With Fallback
```html
<img src="/api/media/profiles/user123/image.jpg" 
     onerror="this.src='/static/images/default-avatar.png'"
     alt="User Profile" />
```

## Testing

### Manual Testing

1. Upload a profile image
2. Check the returned URL starts with `/api/media/`
3. Access the URL in browser
4. Verify image displays correctly
5. Check browser cache headers

### Automated Testing

```rust
#[tokio::test]
async fn test_media_proxy() {
    // Upload test image
    let upload_response = upload_test_image().await;
    
    // Extract path from URL
    let path = extract_path_from_url(&upload_response.url);
    
    // Request through proxy
    let response = get_media_proxy(&path).await;
    
    // Verify response
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/jpeg");
}
```

## Future Roadmap

- [ ] Add authentication for private media
- [ ] Implement in-memory caching
- [ ] Add image transformation capabilities
- [ ] Support for video streaming
- [ ] Integration with CDN
- [ ] Metrics and monitoring dashboard
- [ ] Automatic cleanup of orphaned files
- [ ] Support for multiple storage backends