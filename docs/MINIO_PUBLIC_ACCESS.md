# MinIO Public Access Configuration for Profile Images

## Overview

Profile images uploaded to MinIO need to be publicly accessible for display in the application. This document explains how to configure MinIO to allow public read access for profile images while maintaining security for other content.

## Problem

By default, MinIO objects (files) are private and require authentication to access. This causes profile images to return 403 Forbidden errors when the browser tries to display them.

## Solution

We need to configure MinIO to allow public read access specifically for profile images stored in the `profiles/` directory.

## Implementation Methods

### Method 1: Bucket Policy (Recommended)

Set a bucket policy that allows public read access to all objects in the `profiles/` directory:

```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Principal": {"AWS": ["*"]},
            "Action": ["s3:GetObject"],
            "Resource": ["arn:aws:s3:::slatehub-media/profiles/*"]
        }
    ]
}
```

### Method 2: Object ACL

Set `public-read` ACL on individual objects when uploading. This is implemented in the S3 service:

```rust
// In upload_file method
if key.starts_with("profiles/") {
    request = request.acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead);
}
```

### Method 3: Anonymous Access via MinIO Client

Use the MinIO client to set anonymous access:

```bash
mc anonymous set public local-minio/slatehub-media/profiles/
```

## Setup Instructions

### 1. Using MinIO Console (Web UI)

1. Access MinIO Console at http://localhost:9001
2. Login with credentials:
   - Username: `slatehub`
   - Password: `slatehub123`
3. Navigate to the `slatehub-media` bucket
4. Go to "Access" → "Anonymous"
5. Add a new policy with:
   - Prefix: `profiles/`
   - Access: `readonly`
6. Save the policy

### 2. Using the Fix Script

Run the provided script to automatically configure permissions:

```bash
cd slatehub
./scripts/fix_minio_permissions.sh
```

This script will:
- Install MinIO client if needed
- Configure the MinIO alias
- Set bucket policy for public access
- Fix permissions on existing profile images
- Test public access

### 3. Manual Setup with MinIO Client

Install MinIO client:

```bash
# macOS
brew install minio-mc

# Linux
wget https://dl.min.io/client/mc/release/linux-amd64/mc
chmod +x mc
sudo mv mc /usr/local/bin/
```

Configure and set permissions:

```bash
# Configure MinIO client
mc alias set local-minio http://localhost:9000 slatehub slatehub123

# Create bucket if needed
mc mb local-minio/slatehub-media

# Set anonymous access for profiles directory
mc anonymous set public local-minio/slatehub-media/profiles/

# Verify the policy
mc anonymous get local-minio/slatehub-media
```

## Make Targets for MinIO Public Access

The project includes several convenient make targets for managing MinIO public access:

### Quick Setup

```bash
# Start Docker with automatic public access configuration
make docker-up-public
```

This command starts Docker services and automatically configures MinIO with public access for profiles and organizations folders.

### Setting Public Access

```bash
# Set profiles and organizations folders as public
make minio-public
```

This command:
- Sets public read access for the `profiles/` directory
- Sets public read access for the `organizations/` directory  
- Verifies the current public access policies
- Shows the public URLs for accessing files

### Comprehensive Permission Fix

```bash
# Fix all MinIO permissions (comprehensive)
make minio-fix-permissions
```

This is the most comprehensive command that:
1. Checks if the bucket exists (creates if needed)
2. Sets bucket-level public policies
3. Fixes permissions on all existing files in profiles/ and organizations/
4. Verifies the public access configuration
5. Provides test URLs for validation

Use this command when:
- You have existing files that aren't publicly accessible
- You've manually uploaded files that need public access
- You want to ensure all permissions are correctly set

### Example Workflow

```bash
# Start fresh with public access
make docker-up-public

# Or fix existing setup
make docker-up
make minio-fix-permissions

# Verify public access is working
curl -I http://localhost:9000/slatehub-media/profiles/test/avatar.jpg
```

## Environment Variables

Ensure these environment variables are set correctly:

```env
MINIO_ENDPOINT=http://localhost:9000
MINIO_ACCESS_KEY=slatehub
MINIO_SECRET_KEY=slatehub123
MINIO_BUCKET=slatehub-media
```

## Testing Public Access

### Test with curl

```bash
# Upload a test image first
curl -X PUT http://localhost:9000/slatehub-media/profiles/test.jpg \
  -H "Authorization: AWS4-HMAC-SHA256..." \
  -T test.jpg

# Test public access (no auth)
curl -I http://localhost:9000/slatehub-media/profiles/test.jpg
```

Expected response: `HTTP/1.1 200 OK`

### Test in Browser

Navigate directly to:
```
http://localhost:9000/slatehub-media/profiles/[image-file]
```

The image should display without authentication.

## Troubleshooting

### 403 Forbidden Errors

If you're still getting 403 errors:

1. **Check bucket policy is applied:**
   ```bash
   mc anonymous get local-minio/slatehub-media
   ```

2. **Restart MinIO:**
   ```bash
   docker-compose restart minio
   ```

3. **Check individual file permissions:**
   ```bash
   mc stat local-minio/slatehub-media/profiles/[filename]
   ```

4. **Verify MinIO configuration:**
   - Ensure MinIO is running with the correct ports
   - Check that the bucket exists
   - Verify credentials are correct

### Access Denied in Application

If the application can't access images:

1. **Check the URL format:**
   - Should be: `http://localhost:9000/slatehub-media/profiles/...`
   - Not: `http://localhost:9000/profiles/...`

2. **Verify S3 service configuration:**
   - Check that public-read ACL is being set
   - Ensure the correct endpoint is configured

3. **Browser CORS issues:**
   - MinIO should handle CORS by default
   - If issues persist, configure CORS in MinIO

### MinIO Client Errors

If `mc` commands fail:

1. **Check alias configuration:**
   ```bash
   mc alias list
   ```

2. **Test connection:**
   ```bash
   mc admin info local-minio
   ```

3. **Re-configure alias:**
   ```bash
   mc alias remove local-minio
   mc alias set local-minio http://localhost:9000 slatehub slatehub123
   ```

## Security Considerations

### What's Public

Only files in the `profiles/` directory are publicly accessible:
- Profile images
- Thumbnails

### What Remains Private

All other directories remain private and require authentication:
- `/resumes/` - Private resume files
- `/documents/` - Private documents
- `/media/` - Other private media

### Best Practices

1. **Never store sensitive data in public directories**
2. **Use separate buckets for public vs private content** (future enhancement)
3. **Implement CDN for public images** (production)
4. **Add rate limiting for public endpoints** (production)
5. **Monitor access logs for abuse**

## Production Deployment

For production environments:

1. **Use a CDN:**
   - CloudFront, Cloudflare, or similar
   - Cache public images at edge locations
   - Reduce load on MinIO

2. **Use proper S3 service:**
   - AWS S3 with CloudFront
   - Or MinIO with proper infrastructure

3. **Configure HTTPS:**
   - Use SSL certificates
   - Enforce HTTPS for all requests

4. **Set up monitoring:**
   - Track bandwidth usage
   - Monitor 403/404 errors
   - Alert on unusual access patterns

## Code Changes Made

### S3 Service (`server/src/services/s3.rs`)

1. **Added bucket policy configuration:**
   ```rust
   async fn set_bucket_policy(&self) -> Result<()>
   ```

2. **Added public-read ACL for profile images:**
   ```rust
   if key.starts_with("profiles/") {
       request = request.acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead);
   }
   ```

### Benefits

- Profile images load instantly without authentication
- No need for presigned URLs
- Better user experience
- Reduced server load (no URL generation)
- Browser can cache images effectively

## Related Documentation

- [Profile Image Upload](./PROFILE_IMAGE_UPLOAD.md)
- [Avatar Display Update](./AVATAR_DISPLAY_UPDATE.md)
- [S3 Service Architecture](./S3_SERVICE.md)