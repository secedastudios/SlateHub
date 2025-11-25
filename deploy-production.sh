#!/usr/bin/env bash

# SlateHub Production Deployment Script
# This script helps deploy SlateHub in production mode with proper port 80 handling

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a port is available
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1 ; then
        return 1
    else
        return 0
    fi
}

# Function to check if we need sudo for port 80
needs_sudo_for_port_80() {
    # Check if we're on Linux
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Check if Docker daemon is running as root
        if docker info 2>/dev/null | grep -q "rootless"; then
            return 0  # Rootless Docker, definitely needs special handling
        fi

        # Check if current user can bind to port 80
        if [ "$EUID" -ne 0 ]; then
            # Not root, check if we can bind to port 80
            if ! nc -z 127.0.0.1 80 2>/dev/null; then
                # Port not in use, try to check if we can bind
                if ! python3 -c "import socket; s=socket.socket(); s.bind(('', 80))" 2>/dev/null; then
                    return 0  # Need sudo
                fi
            fi
        fi
    fi
    return 1  # Don't need sudo
}

# Header
echo "========================================"
echo "   SlateHub Production Deployment"
echo "========================================"
echo ""

# Check Docker and Docker Compose
print_info "Checking prerequisites..."

if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed. Please install Docker first."
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    print_error "Docker Compose is not installed. Please install Docker Compose first."
    exit 1
fi

print_info "Docker and Docker Compose are installed ✓"

# Check if Docker daemon is running
if ! docker info > /dev/null 2>&1; then
    print_error "Docker daemon is not running. Please start Docker first."
    exit 1
fi

# Check for production environment file
if [ ! -f ".env.production" ]; then
    print_warn "Production environment file not found."

    if [ -f ".env.production.example" ]; then
        print_info "Creating .env.production from template..."
        cp .env.production.example .env.production
        print_warn "Please edit .env.production with your production values before continuing!"
        print_warn "Especially change all passwords and secret keys!"
        echo ""
        read -p "Press Enter after you've configured .env.production..."
    else
        print_error "No .env.production.example found. Please create .env.production manually."
        exit 1
    fi
fi

# Check critical environment variables
print_info "Checking production configuration..."

# Source the .env.production file to check values
set -a
source .env.production
set +a

# Check if default passwords are still in use
if [[ "$DB_PASSWORD" == *"CHANGE_ME"* ]] || [[ "$JWT_SECRET" == *"CHANGE_ME"* ]]; then
    print_error "Default passwords detected in .env.production!"
    print_error "Please change all CHANGE_ME values to secure passwords."
    exit 1
fi

# Check port availability
PORT=${SERVER_PORT:-80}
print_info "Checking if port $PORT is available..."

if ! check_port $PORT; then
    print_error "Port $PORT is already in use!"
    print_info "Please stop the service using port $PORT or change SERVER_PORT in .env.production"

    # Try to identify what's using the port
    if command -v lsof &> /dev/null; then
        echo ""
        echo "Service using port $PORT:"
        lsof -i :$PORT | grep LISTEN || true
    fi
    exit 1
fi

# Check if we need sudo for port 80
DOCKER_CMD="docker-compose"
if [ "$PORT" -eq 80 ] && needs_sudo_for_port_80; then
    print_warn "Port 80 requires elevated privileges on this system."

    # Check if we're already root
    if [ "$EUID" -ne 0 ]; then
        print_info "Re-running with sudo..."
        exec sudo "$0" "$@"
    fi
    DOCKER_CMD="sudo docker-compose"
fi

# Build the production image
print_info "Building production Docker image..."
$DOCKER_CMD -f docker-compose.prod.yml build

if [ $? -ne 0 ]; then
    print_error "Failed to build Docker image"
    exit 1
fi

# Stop any existing containers
print_info "Stopping any existing containers..."
$DOCKER_CMD -f docker-compose.prod.yml down 2>/dev/null || true

# Start production services
print_info "Starting SlateHub in production mode..."
$DOCKER_CMD -f docker-compose.prod.yml --env-file .env.production up -d

if [ $? -ne 0 ]; then
    print_error "Failed to start services"
    exit 1
fi

# Wait for services to be ready
print_info "Waiting for services to be ready..."
sleep 5

# Health check
print_info "Running health checks..."

# Determine the URL based on port
if [ "$PORT" -eq 80 ]; then
    HEALTH_URL="http://localhost/api/health"
else
    HEALTH_URL="http://localhost:${PORT}/api/health"
fi

# Try health check with retries
MAX_RETRIES=10
RETRY_COUNT=0

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -f -s "$HEALTH_URL" > /dev/null 2>&1; then
        print_info "Health check passed ✓"
        break
    else
        RETRY_COUNT=$((RETRY_COUNT+1))
        if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
            print_error "Health check failed after $MAX_RETRIES attempts"
            print_info "Check logs with: docker-compose -f docker-compose.prod.yml logs"
            exit 1
        fi
        print_warn "Health check attempt $RETRY_COUNT/$MAX_RETRIES failed, retrying..."
        sleep 2
    fi
done

# Success!
echo ""
echo "========================================"
echo -e "${GREEN}   Deployment Successful!${NC}"
echo "========================================"
echo ""

if [ "$PORT" -eq 80 ]; then
    print_info "SlateHub is running at: http://localhost/"
else
    print_info "SlateHub is running at: http://localhost:${PORT}/"
fi

print_info "SurrealDB is available at: http://localhost:8000 (local only)"
print_info "MinIO is available at: http://localhost:9000 (local only)"
echo ""

# Show container status
print_info "Container status:"
docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" | grep -E "NAME|slatehub"

echo ""
print_info "Useful commands:"
echo "  View logs:        docker-compose -f docker-compose.prod.yml logs -f"
echo "  Stop services:    docker-compose -f docker-compose.prod.yml down"
echo "  Restart services: docker-compose -f docker-compose.prod.yml restart"
echo "  View status:      docker-compose -f docker-compose.prod.yml ps"
echo ""

# Security reminders
print_warn "Production Security Checklist:"
echo "  □ Set up HTTPS with SSL/TLS certificates"
echo "  □ Configure a firewall (allow only necessary ports)"
echo "  □ Set up automated backups"
echo "  □ Configure monitoring and alerting"
echo "  □ Review and harden all passwords in .env.production"
echo "  □ Enable rate limiting and DDOS protection"
echo "  □ Set up log rotation and monitoring"
echo ""

print_info "Deployment complete!"
