#!/bin/bash

# Script to start Docker dependencies for SlateHub production
# This can be run independently or called by systemd

set -e

# Get the script directory and project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Change to project root
cd "$PROJECT_ROOT"

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    print_error "Docker is not running. Please start Docker first."
    exit 1
fi

# Check if docker-compose.yml exists
if [ ! -f "docker-compose.yml" ]; then
    print_error "docker-compose.yml not found in $PROJECT_ROOT"
    exit 1
fi

# Start Docker services
print_status "Starting Docker services (SurrealDB and MinIO)..."
docker-compose up -d

# Wait for services to be ready
print_status "Waiting for services to be ready..."

# Wait for SurrealDB to be ready
MAX_RETRIES=30
RETRY_COUNT=0
while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -s -f http://localhost:8000/health > /dev/null 2>&1; then
        print_status "SurrealDB is ready!"
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
        print_error "SurrealDB failed to start within 30 seconds"
        docker-compose logs surrealdb
        exit 1
    fi
    sleep 1
done

# Check MinIO health (using the API endpoint)
RETRY_COUNT=0
while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -s -f http://localhost:9000/minio/health/live > /dev/null 2>&1; then
        print_status "MinIO is ready!"
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
        print_warning "MinIO health check timeout (service may still be starting)"
        # MinIO might still work even if health check fails, so we don't exit
        break
    fi
    sleep 1
done

# Verify services are running
if docker-compose ps | grep -q "Up"; then
    print_status "All Docker services are running:"
    docker-compose ps
else
    print_error "Some services failed to start:"
    docker-compose ps
    exit 1
fi

print_status "Docker dependencies started successfully!"
