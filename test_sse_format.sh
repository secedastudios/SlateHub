#!/bin/bash

# Test script to verify SSE event format from SlateHub server

echo "=========================================="
echo "    SlateHub SSE Format Verification"
echo "=========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if server is running
echo "Checking if server is running on localhost:3000..."
if curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health | grep -q "200"; then
    echo -e "${GREEN}✓ Server is running${NC}"
else
    echo -e "${RED}✗ Server is not running. Please start it with 'cd server && cargo run'${NC}"
    exit 1
fi
echo ""

# Test stats SSE endpoint
echo "=========================================="
echo "Testing /api/sse/stats endpoint"
echo "=========================================="
echo "Capturing 3 events (9 seconds)..."
echo ""

echo -e "${YELLOW}Raw SSE Events:${NC}"
timeout 9 curl -s -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/stats 2>/dev/null | head -20

echo ""
echo "=========================================="
echo "Testing /api/sse/activity endpoint"
echo "=========================================="
echo "Capturing 2 events (10 seconds)..."
echo ""

echo -e "${YELLOW}Raw SSE Events:${NC}"
timeout 10 curl -s -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/activity 2>/dev/null | head -30

echo ""
echo "=========================================="
echo "Expected Format Verification"
echo "=========================================="
echo ""
echo -e "${GREEN}✓ Expected SSE format for Datastar:${NC}"
echo "  event: datastar-signal"
echo "  data: signals {\"key\": value, ...}"
echo ""
echo -e "${GREEN}✓ Stats signals format:${NC}"
echo "  signals {\"projectCount\": 1234, \"userCount\": 5678, \"connectionCount\": 9012}"
echo ""
echo -e "${GREEN}✓ Activity signals format:${NC}"
echo "  signals {\"activities\": [...]}"
echo ""

# Parse and verify format
echo "=========================================="
echo "Format Validation"
echo "=========================================="
echo ""

echo "Checking stats endpoint format..."
STATS_OUTPUT=$(timeout 3 curl -s -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/stats 2>/dev/null | head -10)

if echo "$STATS_OUTPUT" | grep -q "event: datastar-signal"; then
    echo -e "${GREEN}✓ Correct event type: datastar-signal${NC}"
else
    echo -e "${RED}✗ Incorrect event type (should be 'datastar-signal')${NC}"
fi

if echo "$STATS_OUTPUT" | grep -q "data: signals {"; then
    echo -e "${GREEN}✓ Correct data format: signals {...}${NC}"
else
    echo -e "${RED}✗ Incorrect data format (should start with 'signals {')${NC}"
fi

if echo "$STATS_OUTPUT" | grep -q "projectCount"; then
    echo -e "${GREEN}✓ Contains projectCount signal${NC}"
else
    echo -e "${RED}✗ Missing projectCount signal${NC}"
fi

echo ""
echo "Checking activity endpoint format..."
ACTIVITY_OUTPUT=$(timeout 5 curl -s -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/activity 2>/dev/null | head -10)

if echo "$ACTIVITY_OUTPUT" | grep -q "event: datastar-signal"; then
    echo -e "${GREEN}✓ Correct event type: datastar-signal${NC}"
else
    echo -e "${RED}✗ Incorrect event type (should be 'datastar-signal')${NC}"
fi

if echo "$ACTIVITY_OUTPUT" | grep -q "data: signals {"; then
    echo -e "${GREEN}✓ Correct data format: signals {...}${NC}"
else
    echo -e "${RED}✗ Incorrect data format (should start with 'signals {')${NC}"
fi

if echo "$ACTIVITY_OUTPUT" | grep -q "activities"; then
    echo -e "${GREEN}✓ Contains activities signal${NC}"
else
    echo -e "${RED}✗ Missing activities signal${NC}"
fi

echo ""
echo "=========================================="
echo -e "${GREEN}Test Complete!${NC}"
echo "=========================================="
echo ""
echo "To see the full experience:"
echo "1. Open http://localhost:3000 in your browser"
echo "2. Watch the Platform Statistics section update every 3 seconds"
echo "3. Watch the Recent Activity section update every 5 seconds"
echo ""
echo "For debugging, open http://localhost:3000/static/test-sse.html"
