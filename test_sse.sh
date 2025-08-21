#!/bin/bash

# Test script for SSE endpoints

echo "Testing SlateHub SSE Endpoints"
echo "=============================="
echo ""

# Check if server is running
echo "Checking if server is running on localhost:3000..."
if ! curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health | grep -q "200"; then
    echo "❌ Server is not running. Please start the server first with 'cargo run'"
    exit 1
fi
echo "✅ Server is running"
echo ""

# Test stats SSE endpoint
echo "Testing /api/sse/stats endpoint (will run for 10 seconds)..."
echo "--------------------------------------------------------------"
timeout 10 curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/stats 2>/dev/null | while IFS= read -r line; do
    if [[ $line == data:* ]]; then
        echo "📊 Stats Update: ${line#data: }"
    fi
done
echo ""

# Test activity SSE endpoint
echo "Testing /api/sse/activity endpoint (will run for 10 seconds)..."
echo "----------------------------------------------------------------"
timeout 10 curl -N -H "Accept: text/event-stream" http://localhost:3000/api/sse/activity 2>/dev/null | while IFS= read -r line; do
    if [[ $line == data:* ]]; then
        echo "📝 Activity Update: ${line#data: }"
    fi
done
echo ""

echo "✅ SSE test complete!"
echo ""
echo "To see the full experience, open http://localhost:3000 in your browser"
echo "and watch the Platform Statistics and Recent Activity sections update in real-time!"
