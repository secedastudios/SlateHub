#!/bin/bash

# Fix MinIO permissions for organization logos
# This script sets up public read access for organization logos in MinIO

set -e

echo "🔧 Fixing MinIO permissions for organization logos..."

# MinIO configuration
MINIO_ENDPOINT=${MINIO_ENDPOINT:-"http://localhost:9000"}
MINIO_ACCESS_KEY=${MINIO_ACCESS_KEY:-"slatehub"}
MINIO_SECRET_KEY=${MINIO_SECRET_KEY:-"slatehub123"}
MINIO_BUCKET=${MINIO_BUCKET:-"slatehub-media"}

# Check if mc (MinIO client) is installed
if ! command -v mc &> /dev/null; then
    echo "⚠️  MinIO client (mc) not found. Installing..."

    # Detect OS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        if command -v brew &> /dev/null; then
            brew install minio-mc
        else
            echo "❌ Homebrew not found. Please install MinIO client manually:"
            echo "   brew install minio-mc"
            exit 1
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Linux
        wget https://dl.min.io/client/mc/release/linux-amd64/mc -q
        chmod +x mc
        sudo mv mc /usr/local/bin/
        echo "✅ MinIO client installed"
    else
        echo "❌ Unsupported OS. Please install MinIO client manually"
        exit 1
    fi
fi

# Configure MinIO client alias
echo "📝 Configuring MinIO client..."
mc alias set local-minio "$MINIO_ENDPOINT" "$MINIO_ACCESS_KEY" "$MINIO_SECRET_KEY" --api S3v4

# Check if bucket exists
if ! mc ls local-minio | grep -q "$MINIO_BUCKET"; then
    echo "📦 Creating bucket: $MINIO_BUCKET"
    mc mb "local-minio/$MINIO_BUCKET"
fi

# Set anonymous access policy for profiles directory (if not already set)
echo "🔓 Setting public access for profiles directory..."
mc anonymous set public "local-minio/$MINIO_BUCKET/profiles/" 2>/dev/null || true

# Set anonymous access policy for organizations directory
echo "🔓 Setting public access for organizations directory..."
mc anonymous set public "local-minio/$MINIO_BUCKET/organizations/" 2>/dev/null || true

# Verify the policies
echo ""
echo "📋 Current anonymous policies:"
mc anonymous get "local-minio/$MINIO_BUCKET" | grep -E "(profiles|organizations)" || echo "No policies found"

# Test with a dummy file upload
echo ""
echo "🧪 Testing public access..."

# Create a test file
echo "test" > /tmp/test-org-logo.txt

# Upload test file to organizations directory
mc cp /tmp/test-org-logo.txt "local-minio/$MINIO_BUCKET/organizations/test/test-logo.txt" 2>/dev/null || true

# Test public access (no auth)
if curl -s -o /dev/null -w "%{http_code}" "$MINIO_ENDPOINT/$MINIO_BUCKET/organizations/test/test-logo.txt" | grep -q "200"; then
    echo "✅ Public access for organizations directory is working!"
else
    echo "⚠️  Public access test failed. You may need to restart MinIO:"
    echo "   docker-compose restart minio"
fi

# Clean up test file
mc rm "local-minio/$MINIO_BUCKET/organizations/test/test-logo.txt" 2>/dev/null || true
rm /tmp/test-org-logo.txt 2>/dev/null || true

echo ""
echo "✨ MinIO permissions fixed!"
echo ""
echo "📝 Summary:"
echo "   - Bucket: $MINIO_BUCKET"
echo "   - Public directories: /profiles/*, /organizations/*"
echo "   - Endpoint: $MINIO_ENDPOINT"
echo ""
echo "💡 Tips:"
echo "   - Organization logos will be accessible at: $MINIO_ENDPOINT/$MINIO_BUCKET/organizations/{org_slug}/"
echo "   - Profile images will be accessible at: $MINIO_ENDPOINT/$MINIO_BUCKET/profiles/{user_id}/"
echo "   - If images still return 403, restart MinIO: docker-compose restart minio"
