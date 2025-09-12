#!/bin/bash

# Test script to verify organization types are properly loaded in the database

echo "========================================="
echo "Testing Organization Types in Database"
echo "========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Database connection details
DB_HOST="localhost"
DB_PORT="8000"
DB_USER="root"
DB_PASS="root"
DB_NS="slatehub"
DB_NAME="main"

echo "Database Configuration:"
echo "  Host: $DB_HOST:$DB_PORT"
echo "  Namespace: $DB_NS"
echo "  Database: $DB_NAME"
echo ""

# Function to execute SQL query
execute_query() {
    local query="$1"
    curl -s -X POST \
        -H "Accept: application/json" \
        -H "Surreal-NS: $DB_NS" \
        -H "Surreal-DB: $DB_NAME" \
        --user "$DB_USER:$DB_PASS" \
        --data "$query" \
        "http://$DB_HOST:$DB_PORT/sql"
}

# Test 1: Check if organization_type table exists
echo "Test 1: Checking if organization_type table exists..."
result=$(execute_query "INFO FOR TABLE organization_type;")
if echo "$result" | grep -q '"status":"OK"'; then
    echo -e "${GREEN}✓ Table organization_type exists${NC}"
else
    echo -e "${RED}✗ Table organization_type does not exist${NC}"
    echo "Response: $result"
    exit 1
fi
echo ""

# Test 2: Count organization types
echo "Test 2: Counting organization types..."
result=$(execute_query "SELECT count() as total FROM organization_type GROUP ALL;")
count=$(echo "$result" | grep -o '"total":[0-9]*' | grep -o '[0-9]*')
if [ -n "$count" ] && [ "$count" -gt 0 ]; then
    echo -e "${GREEN}✓ Found $count organization types${NC}"
else
    echo -e "${RED}✗ No organization types found${NC}"
    echo "Response: $result"
    exit 1
fi
echo ""

# Test 3: Fetch organization types with the exact query used in the code
echo "Test 3: Testing the exact query used in the application..."
query="SELECT meta::id(id) as id, name FROM organization_type ORDER BY name;"
result=$(execute_query "$query")
if echo "$result" | grep -q '"status":"OK"'; then
    echo -e "${GREEN}✓ Query executed successfully${NC}"

    # Extract and display the organization types
    echo ""
    echo "Organization types found:"
    echo "$result" | python3 -c "
import sys
import json
data = json.load(sys.stdin)
if data and len(data) > 0 and 'result' in data[0]:
    for item in data[0]['result'][:10]:  # Show first 10
        print(f\"  - ID: {item['id']}, Name: {item['name']}\")
    if len(data[0]['result']) > 10:
        print(f\"  ... and {len(data[0]['result']) - 10} more\")
" 2>/dev/null || echo "$result" | grep -o '"name":"[^"]*"' | head -10
else
    echo -e "${RED}✗ Query failed${NC}"
    echo "Response: $result"
    exit 1
fi
echo ""

# Test 4: Verify expected organization types are present
echo "Test 4: Verifying expected organization types..."
expected_types=("production_company" "film_studio" "tv_production_company" "animation_studio" "talent_agency")
missing_types=()

for expected in "${expected_types[@]}"; do
    result=$(execute_query "SELECT name FROM organization_type WHERE name = '$expected';")
    if echo "$result" | grep -q "\"$expected\""; then
        echo -e "${GREEN}  ✓ Found: $expected${NC}"
    else
        echo -e "${RED}  ✗ Missing: $expected${NC}"
        missing_types+=("$expected")
    fi
done

if [ ${#missing_types[@]} -eq 0 ]; then
    echo -e "${GREEN}✓ All expected organization types are present${NC}"
else
    echo -e "${YELLOW}⚠ Some organization types are missing. You may need to run: make db-init${NC}"
fi
echo ""

# Test 5: Test the query without meta::id to see the difference
echo "Test 5: Comparing query results with and without meta::id()..."
query1="SELECT id, name FROM organization_type LIMIT 1;"
query2="SELECT meta::id(id) as id, name FROM organization_type LIMIT 1;"

echo "Without meta::id():"
result1=$(execute_query "$query1")
echo "$result1" | python3 -m json.tool 2>/dev/null | grep -A2 '"result"' | tail -2 || echo "$result1"

echo ""
echo "With meta::id():"
result2=$(execute_query "$query2")
echo "$result2" | python3 -m json.tool 2>/dev/null | grep -A2 '"result"' | tail -2 || echo "$result2"

echo ""
echo "========================================="
echo "Test Summary"
echo "========================================="

# Final summary
if [ ${#missing_types[@]} -eq 0 ] && [ -n "$count" ] && [ "$count" -gt 0 ]; then
    echo -e "${GREEN}✓ All tests passed! Organization types are properly configured.${NC}"
    echo ""
    echo "The database contains $count organization types and they can be queried successfully."
    echo "The new organization form should now show the types in the dropdown."
else
    echo -e "${YELLOW}⚠ Some issues were found.${NC}"
    echo ""
    echo "Recommended action:"
    echo "  1. Run: make db-init"
    echo "  2. Restart the server: make server-run"
    echo "  3. Try accessing /orgs/new again"
fi
