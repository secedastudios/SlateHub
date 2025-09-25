# Organization Logo Upload Feature

## Overview

Organizations in SlateHub can now upload and manage their logos, providing visual branding for their profiles. This feature supports standard image formats (JPEG, PNG, WebP) as well as SVG for vector logos, with automatic cropping and resizing capabilities.

## Features

- **Multiple Format Support**: JPEG, PNG, WebP, and SVG
- **Interactive Crop Tool**: For raster images, users can zoom and position their logo
- **SVG Support**: Vector logos maintain quality at any size
- **Automatic Thumbnail Generation**: Creates optimized thumbnails for list views
- **Public Access**: Logos are publicly accessible via MinIO/S3
- **Permission-Based**: Only organization owners and admins can update logos

## Technical Implementation

### Backend Components

#### 1. Media Routes (`server/src/routes/media.rs`)

Two endpoints handle logo uploads:

```rust
// Upload with slug in multipart form
POST /api/media/upload/organization-logo

// Upload with slug in URL path (recommended)
POST /api/media/upload/organization-logo/{org_slug}

// Get organization logo URL
GET /api/media/organization-logo/{org_slug}
```

#### 2. Image Processing

- **Raster Images**: Processed to 400x400px square format with Lanczos3 resampling
- **SVG Files**: Stored as-is for maximum quality
- **Thumbnails**: 100x100px JPEG thumbnails generated for all formats

#### 3. Storage Structure

Logos are stored in MinIO/S3 with the following structure:
```
organizations/
├── {org_slug}/
│   ├── logo_{ulid}.jpg       # Main logo (raster)
│   ├── logo_{ulid}.svg       # Main logo (SVG)
│   └── thumb_{ulid}.jpg      # Thumbnail
```

### Frontend Components

#### 1. Upload Interface (`server/static/js/organization-logo-upload.js`)

The `OrganizationLogoUploader` class provides:
- Drag-and-drop file selection
- File browse button
- Real-time preview
- Interactive crop controls (for raster images)
- Upload progress indicator
- Success/error notifications

#### 2. Template Integration

Edit organization page includes the upload component:

```html
<div id="organization-logo-upload" data-component="image-upload" data-state="ready">
    <!-- Logo upload component initialized here -->
</div>

<script>
    const orgSlug = '{{ organization.slug }}';
    new OrganizationLogoUploader('organization-logo-upload', orgSlug);
</script>
```

### Database Schema

The organization table stores the logo URL directly:

```sql
DEFINE FIELD logo ON organization TYPE option<string> PERMISSIONS FULL;
```

## Usage

### For Users

1. Navigate to organization settings/edit page
2. Click "Browse Files" or drag and drop a logo image
3. For raster images:
   - Use zoom slider to adjust size
   - Drag image to reposition
   - Click "Upload Logo" when satisfied
4. For SVG files:
   - Preview displays immediately
   - Click "Upload Logo" to save

### For Developers

#### Adding Logo Upload to a Page

```javascript
// Initialize the uploader with organization slug
const uploader = new OrganizationLogoUploader('container-id', 'org-slug');
```

#### Displaying Organization Logos

```html
<!-- In templates -->
{% if organization.logo.is_some() %}
    <img 
        src="{{ organization.logo.as_ref().unwrap() }}" 
        alt="{{ organization.name }} logo"
        data-organization-logo
    />
{% endif %}
```

## Security Considerations

### Permission Checks

Only organization owners and admins can upload logos:

```rust
// Check membership role before allowing upload
SELECT * FROM membership 
WHERE person = person:{user_id} 
AND organization.slug = '{org_slug}' 
AND role IN ['owner', 'admin']
```

### File Validation

- **Size Limit**: 10MB maximum
- **Type Validation**: Only accepted image formats
- **Content Verification**: File content validated against MIME type

## Public Access Configuration

### MinIO Bucket Policy

Organization logos are publicly accessible through bucket policies:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Principal": {"AWS": ["*"]},
            "Action": ["s3:GetObject"],
            "Resource": ["arn:aws:s3:::slatehub-media/organizations/*"]
        }
    ]
}
```

### Setup Script

Run the provided script to configure MinIO permissions:

```bash
./scripts/fix_minio_org_permissions.sh
```

This script:
- Configures MinIO client
- Sets public read access for organization directories
- Verifies the configuration
- Tests public access

## API Reference

### Upload Organization Logo

**Endpoint**: `POST /api/media/upload/organization-logo/{org_slug}`

**Headers**:
- Cookie with authentication token

**Multipart Form Data**:
- `image` or `file`: The logo image file

**Query Parameters** (optional, for raster images):
- `crop_x`: Horizontal crop position (0-1)
- `crop_y`: Vertical crop position (0-1)
- `crop_zoom`: Zoom level (1.0-3.0)

**Response**:
```json
{
    "media_id": "01HG3JKMNP7Q2Z8A9F1B3C4D5E",
    "url": "http://localhost:9000/slatehub-media/organizations/acme/logo_01HG3JKMNP7Q2Z8A9F1B3C4D5E.jpg",
    "thumbnail_url": "http://localhost:9000/slatehub-media/organizations/acme/thumb_01HG3JKMNP7Q2Z8A9F1B3C4D5E.jpg"
}
```

### Get Organization Logo URL

**Endpoint**: `GET /api/media/organization-logo/{org_slug}`

**Response**:
```json
{
    "url": "http://localhost:9000/slatehub-media/organizations/acme/logo_01HG3JKMNP7Q2Z8A9F1B3C4D5E.jpg",
    "has_logo": true
}
```

## Troubleshooting

### Logo Not Displaying

1. **Check Public Access**: Ensure MinIO bucket policy is configured
2. **Verify URL**: Check that the logo URL is correctly stored in the database
3. **Browser Console**: Look for 403 Forbidden or CORS errors
4. **Restart MinIO**: `docker-compose restart minio`

### Upload Failures

1. **Check File Size**: Must be under 10MB
2. **Verify Permissions**: User must be owner or admin
3. **Check File Type**: Must be JPEG, PNG, WebP, or SVG
4. **Network Issues**: Check MinIO service is running

### SVG Issues

- SVG files are stored as-is without processing
- Thumbnails for SVG use a placeholder (full rasterization pending)
- Some older browsers may not support SVG in img tags

## Future Enhancements

1. **CDN Integration**: Serve logos through CloudFront or similar
2. **Image Optimization**: Automatic format conversion (WebP, AVIF)
3. **SVG Rasterization**: Generate proper thumbnails from SVG files
4. **Aspect Ratio Options**: Support non-square logos
5. **Multiple Logo Variants**: Light/dark mode versions
6. **Logo History**: Track and revert to previous logos
7. **Batch Upload**: Support multiple image variants at once
8. **AI Enhancement**: Auto-remove backgrounds, upscale low-res logos
9. **Brand Kit**: Store multiple brand assets (logos, colors, fonts)
10. **Logo Animation**: Support animated SVG or GIF logos

## Related Documentation

- [Profile Image Upload](./PROFILE_IMAGE_UPLOAD.md)
- [MinIO Public Access](./MINIO_PUBLIC_ACCESS.md)
- [S3 Service Architecture](./S3_SERVICE.md)
- [HTML & CSS Guidelines](./HTML_CSS_GUIDELINES.md)

## Code Examples

### Custom Logo Display Component

```javascript
class LogoDisplay {
    constructor(orgSlug) {
        this.orgSlug = orgSlug;
        this.loadLogo();
    }
    
    async loadLogo() {
        const response = await fetch(`/api/media/organization-logo/${this.orgSlug}`);
        const data = await response.json();
        
        if (data.has_logo) {
            this.displayLogo(data.url);
        } else {
            this.displayPlaceholder();
        }
    }
    
    displayLogo(url) {
        const img = document.createElement('img');
        img.src = url;
        img.alt = 'Organization logo';
        // Add to DOM
    }
    
    displayPlaceholder() {
        // Show default placeholder
    }
}
```

### Server-Side Logo Validation

```rust
fn validate_logo_file(data: &[u8], content_type: &str) -> Result<(), Error> {
    // Check file signature matches content type
    match content_type {
        "image/jpeg" => {
            if !data.starts_with(&[0xFF, 0xD8, 0xFF]) {
                return Err(Error::bad_request("Invalid JPEG file"));
            }
        }
        "image/png" => {
            if !data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                return Err(Error::bad_request("Invalid PNG file"));
            }
        }
        "image/svg+xml" => {
            // Basic SVG validation
            let content = String::from_utf8_lossy(data);
            if !content.contains("<svg") {
                return Err(Error::bad_request("Invalid SVG file"));
            }
        }
        _ => {}
    }
    Ok(())
}
```
