#!/bin/bash

# SlateHub Test Runner Script
# This script ensures proper setup and teardown of test environment

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
TEST_DOCKER_COMPOSE="docker-compose.test.yml"
TEST_DATA_DIR="db/test-data"
TEST_FILES_DIR="db/test-files"
SERVER_DIR="server"

# Test environment variables
export DATABASE_URL="ws://localhost:8100/rpc"
export DATABASE_USER="root"
export DATABASE_PASS="root"
export DATABASE_NS="slatehub-test"
export DATABASE_DB="test"
export MINIO_ENDPOINT="http://localhost:9100"
export MINIO_ACCESS_KEY="slatehub-test"
export MINIO_SECRET_KEY="slatehub-test123"
export MINIO_BUCKET="slatehub-test-media"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[TEST]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Cleanup function
cleanup() {
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        print_error "Tests failed with exit code: $exit_code"
    fi

    print_status "Cleaning up test environment..."

    # Stop test containers
    docker-compose -f $TEST_DOCKER_COMPOSE down 2>/dev/null || true

    # Clean test data
    rm -rf $TEST_DATA_DIR/* $TEST_FILES_DIR/* 2>/dev/null || true

    print_status "Cleanup complete"

    exit $exit_code
}

# Setup cleanup on exit
trap cleanup EXIT INT TERM

# Function to wait for service
wait_for_service() {
    local url=$1
    local service_name=$2
    local max_attempts=30
    local attempt=1

    print_status "Waiting for $service_name to be ready..."

    while [ $attempt -le $max_attempts ]; do
        if curl -s -f -o /dev/null "$url"; then
            print_status "$service_name is ready!"
            return 0
        fi

        echo -n "."
        sleep 1
        attempt=$((attempt + 1))
    done

    echo ""
    print_error "$service_name failed to start after $max_attempts seconds"
    return 1
}

# Function to setup test environment
setup_test_env() {
    print_status "Setting up test environment..."

    # Create test directories
    mkdir -p $TEST_DATA_DIR $TEST_FILES_DIR

    # Start test containers
    print_status "Starting test Docker containers..."
    docker-compose -f $TEST_DOCKER_COMPOSE up -d

    # Wait for services to be ready
    wait_for_service "http://localhost:8100/health" "SurrealDB Test"
    wait_for_service "http://localhost:9100/minio/health/live" "MinIO Test"

    print_status "Test environment is ready!"
}

# Function to run tests
run_tests() {
    local test_type=$1
    local test_args=""

    case $test_type in
        "all")
            print_status "Running all tests..."
            test_args="--all"
            ;;
        "unit")
            print_status "Running unit tests..."
            test_args="--lib"
            ;;
        "integration")
            print_status "Running integration tests..."
            test_args="--test '*'"
            ;;
        "file")
            if [ -z "$2" ]; then
                print_error "Test file name required"
                exit 1
            fi
            print_status "Running test file: $2..."
            test_args="$2"
            ;;
        *)
            print_error "Unknown test type: $test_type"
            echo "Usage: $0 [all|unit|integration|file <name>]"
            exit 1
            ;;
    esac

    # Change to server directory
    cd $SERVER_DIR

    # Run tests with single thread to avoid conflicts
    cargo test $test_args -- --test-threads=1 --nocapture

    cd ..
}

# Main script
main() {
    local test_type=${1:-"all"}
    local test_file=${2:-""}

    print_status "SlateHub Test Runner"
    print_status "Test type: $test_type"

    # Setup environment
    setup_test_env

    # Run tests
    if [ "$test_type" = "file" ]; then
        run_tests "$test_type" "$test_file"
    else
        run_tests "$test_type"
    fi

    print_status "All tests completed successfully!"
}

# Check if we're in the right directory
if [ ! -f "Makefile" ] || [ ! -d "server" ]; then
    print_error "This script must be run from the SlateHub project root directory"
    exit 1
fi

# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
    print_error "Docker is not running. Please start Docker and try again."
    exit 1
fi

# Run main function with arguments
main "$@"
