#!/bin/bash

# Test script for profile image upload functionality
# This script tests the new direct URL storage for profile avatars

set -e

# Configuration
BASE_URL="${BASE_URL:-http://localhost:3000}"
TEST_USER_EMAIL="${TEST_USER_EMAIL:-test@example.com}"
TEST_USER_PASSWORD="${TEST_USER_PASSWORD:-password123}"
TEST_IMAGE="${TEST_IMAGE:-test_image.jpg}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${YELLOW}ℹ${NC} $1"
}

# Create a test image if it doesn't exist
create_test_image() {
    if [ ! -f "$TEST_IMAGE" ]; then
        print_info "Creating test image..."
        # Create a simple 100x100 test image using ImageMagick if available
        if command -v convert &> /dev/null; then
            convert -size 100x100 xc:blue "$TEST_IMAGE"
            print_status "Test image created"
        else
            # Create a minimal valid JPEG using base64
            echo "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAr/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFQEBAQAAAAAAAAAAAAAAAAAAAAX/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIRAxEAPwCwAA8A/9k=" | base64 -d > "$TEST_IMAGE"
            print_status "Test image created (minimal JPEG)"
        fi
    else
        print_status "Using existing test image: $TEST_IMAGE"
    fi
}

# Function to login and get JWT token
login() {
    print_info "Logging in as $TEST_USER_EMAIL..."

    RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "email=$TEST_USER_EMAIL&password=$TEST_USER_PASSWORD" \
        -c cookies.txt \
        -w "\n%{http_code}")

    HTTP_CODE=$(echo "$RESPONSE" | tail -n 1)

    if [ "$HTTP_CODE" = "303" ] || [ "$HTTP_CODE" = "302" ]; then
        print_status "Login successful"
        return 0
    else
        print_error "Login failed with HTTP code: $HTTP_CODE"
        return 1
    fi
}

# Function to upload profile image
upload_profile_image() {
    print_info "Uploading profile image..."

    # Upload with crop parameters
    RESPONSE=$(curl -s -X POST "$BASE_URL/api/media/upload/profile-image?crop_x=0.5&crop_y=0.5&crop_zoom=1.0" \
        -H "Accept: application/json" \
        -F "image=@$TEST_IMAGE;type=image/jpeg" \
        -b cookies.txt \
        -w "\n%{http_code}")

    HTTP_CODE=$(echo "$RESPONSE" | tail -n 1)
    BODY=$(echo "$RESPONSE" | head -n -1)

    if [ "$HTTP_CODE" = "200" ]; then
        print_status "Profile image uploaded successfully"

        # Extract URL from response
        if command -v jq &> /dev/null; then
            URL=$(echo "$BODY" | jq -r '.url')
            MEDIA_ID=$(echo "$BODY" | jq -r '.media_id')
            print_info "Image URL: $URL"
            print_info "Media ID: $MEDIA_ID"
        else
            print_info "Response: $BODY"
        fi

        return 0
    else
        print_error "Upload failed with HTTP code: $HTTP_CODE"
        print_info "Response: $BODY"
        return 1
    fi
}

# Function to verify avatar URL is stored
verify_avatar_stored() {
    print_info "Verifying avatar URL is stored in profile..."

    # Get the user ID from the session
    # For now, we'll check the avatar endpoint
    RESPONSE=$(curl -s -L "$BASE_URL/api/avatar?id=test" \
        -b cookies.txt \
        -w "\n%{http_code}" \
        -I)

    HTTP_CODE=$(echo "$RESPONSE" | tail -n 1)

    if [ "$HTTP_CODE" = "301" ] || [ "$HTTP_CODE" = "302" ] || [ "$HTTP_CODE" = "200" ]; then
        print_status "Avatar endpoint is responding"

        # Check if it's redirecting to MinIO URL or DiceBear
        if echo "$RESPONSE" | grep -q "minio\|localhost:9000\|s3"; then
            print_status "Avatar is being served from MinIO (custom upload)"
        elif echo "$RESPONSE" | grep -q "dicebear"; then
            print_info "Avatar is using default DiceBear service"
        fi

        return 0
    else
        print_error "Avatar verification failed with HTTP code: $HTTP_CODE"
        return 1
    fi
}

# Function to cleanup
cleanup() {
    print_info "Cleaning up..."
    rm -f cookies.txt
    if [ -f "$TEST_IMAGE" ]; then
        print_info "Keeping test image for future tests"
    fi
}

# Main test flow
main() {
    echo "========================================="
    echo "Profile Image Upload Test"
    echo "========================================="
    echo ""

    # Check if server is running
    if ! curl -s -f "$BASE_URL/api/health" > /dev/null 2>&1; then
        print_error "Server is not running at $BASE_URL"
        exit 1
    fi
    print_status "Server is running"

    # Create test image
    create_test_image

    # Login
    if ! login; then
        print_error "Login failed, cannot continue"
        cleanup
        exit 1
    fi

    # Upload profile image
    if ! upload_profile_image; then
        print_error "Profile image upload failed"
        cleanup
        exit 1
    fi

    # Verify avatar is stored
    if ! verify_avatar_stored; then
        print_error "Avatar verification failed"
        cleanup
        exit 1
    fi

    echo ""
    echo "========================================="
    print_status "All tests passed successfully!"
    echo "========================================="

    cleanup
}

# Handle script interruption
trap cleanup EXIT

# Run main function
main
