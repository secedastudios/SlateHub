#!/bin/bash

# Test script for organization creation
# This script tests the organization creation endpoint to verify the datetime serialization fix

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="http://localhost:3000"
TEST_USER_EMAIL="test@example.com"
TEST_USER_PASSWORD="TestPassword123!"
TEST_USER_USERNAME="testuser"

echo -e "${YELLOW}Organization Creation Test Script${NC}"
echo "=================================="
echo ""

# Function to check if server is running
check_server() {
    echo -n "Checking if server is running at $BASE_URL... "
    if curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/health" | grep -q "200"; then
        echo -e "${GREEN}OK${NC}"
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        echo "Please start the server first with: cd server && cargo run"
        exit 1
    fi
}

# Function to create a test user
create_test_user() {
    echo -n "Creating test user... "

    SIGNUP_RESPONSE=$(curl -s -X POST "$BASE_URL/api/auth/signup" \
        -H "Content-Type: application/json" \
        -d "{
            \"username\": \"$TEST_USER_USERNAME\",
            \"email\": \"$TEST_USER_EMAIL\",
            \"password\": \"$TEST_USER_PASSWORD\"
        }")

    if echo "$SIGNUP_RESPONSE" | grep -q "token"; then
        echo -e "${GREEN}OK${NC}"
        TOKEN=$(echo "$SIGNUP_RESPONSE" | grep -o '"token":"[^"]*' | sed 's/"token":"//')
        return 0
    elif echo "$SIGNUP_RESPONSE" | grep -q "already exists"; then
        echo -e "${YELLOW}User already exists, attempting login...${NC}"
        login_test_user
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        echo "Response: $SIGNUP_RESPONSE"
        return 1
    fi
}

# Function to login test user
login_test_user() {
    echo -n "Logging in test user... "

    LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/api/auth/signin" \
        -H "Content-Type: application/json" \
        -d "{
            \"identifier\": \"$TEST_USER_USERNAME\",
            \"password\": \"$TEST_USER_PASSWORD\"
        }")

    if echo "$LOGIN_RESPONSE" | grep -q "token"; then
        echo -e "${GREEN}OK${NC}"
        TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"token":"[^"]*' | sed 's/"token":"//')
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        echo "Response: $LOGIN_RESPONSE"
        exit 1
    fi
}

# Function to test organization creation
test_org_creation() {
    echo ""
    echo "Testing organization creation..."
    echo "--------------------------------"

    # Generate unique org name and slug
    TIMESTAMP=$(date +%s)
    ORG_NAME="Test Organization $TIMESTAMP"
    ORG_SLUG="test-org-$TIMESTAMP"

    echo "Creating organization: $ORG_NAME (slug: $ORG_SLUG)"

    # Create organization via form submission (mimicking the web form)
    CREATE_RESPONSE=$(curl -s -X POST "$BASE_URL/orgs/new" \
        -H "Cookie: token=$TOKEN" \
        -H "Content-Type: application/x-www-form-urlencoded" \
        --data-urlencode "name=$ORG_NAME" \
        --data-urlencode "slug=$ORG_SLUG" \
        --data-urlencode "org_type=studio" \
        --data-urlencode "description=Test organization for datetime serialization" \
        --data-urlencode "location=Los Angeles, CA" \
        --data-urlencode "website=https://example.com" \
        --data-urlencode "contact_email=contact@example.com" \
        --data-urlencode "phone=555-1234" \
        --data-urlencode "services=vfx" \
        --data-urlencode "services=animation" \
        --data-urlencode "founded_year=2024" \
        --data-urlencode "employees_count=10" \
        --data-urlencode "public=true" \
        -w "\nHTTP_STATUS:%{http_code}")

    HTTP_STATUS=$(echo "$CREATE_RESPONSE" | grep "HTTP_STATUS" | cut -d: -f2)

    if [ "$HTTP_STATUS" = "303" ] || [ "$HTTP_STATUS" = "302" ]; then
        echo -e "${GREEN}✓ Organization created successfully (redirect status: $HTTP_STATUS)${NC}"

        # Try to fetch the created organization
        echo -n "Fetching created organization... "
        ORG_RESPONSE=$(curl -s "$BASE_URL/orgs/$ORG_SLUG" \
            -H "Cookie: token=$TOKEN")

        if echo "$ORG_RESPONSE" | grep -q "$ORG_NAME"; then
            echo -e "${GREEN}OK${NC}"

            # Check if datetime fields are present
            echo -n "Checking datetime fields... "
            if echo "$ORG_RESPONSE" | grep -q "Created"; then
                echo -e "${GREEN}OK - Datetime fields are rendered${NC}"
            else
                echo -e "${YELLOW}WARNING - Could not verify datetime fields in response${NC}"
            fi
        else
            echo -e "${RED}FAILED - Could not fetch organization${NC}"
        fi

        return 0
    elif [ "$HTTP_STATUS" = "500" ]; then
        echo -e "${RED}✗ Server error (500) - datetime serialization issue likely still present${NC}"
        echo "Response body (first 500 chars):"
        echo "$CREATE_RESPONSE" | head -c 500
        return 1
    else
        echo -e "${RED}✗ Unexpected status code: $HTTP_STATUS${NC}"
        echo "Response body (first 500 chars):"
        echo "$CREATE_RESPONSE" | head -c 500
        return 1
    fi
}

# Function to cleanup test data
cleanup() {
    echo ""
    echo "Cleanup note: Test organizations can be deleted manually through the UI"
}

# Main execution
main() {
    check_server

    echo ""
    create_test_user || login_test_user

    test_org_creation

    cleanup

    echo ""
    echo "=================================="
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}Test completed successfully!${NC}"
    else
        echo -e "${RED}Test failed!${NC}"
        exit 1
    fi
}

# Run the main function
main
