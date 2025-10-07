#!/bin/bash

# SlateHub Production Deployment Script
# This script handles the complete production deployment setup

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_DIR="$PROJECT_ROOT/server"
BINARY_PATH="$SERVER_DIR/target/release/slatehub"
PID_FILE="$PROJECT_ROOT/slatehub.pid"
LOG_FILE="$PROJECT_ROOT/slatehub.log"
ENV_FILE="$PROJECT_ROOT/.env"
SYSTEMD_SERVICE="slatehub.service"

# Functions
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

check_prerequisites() {
    print_status "Checking prerequisites..."

    # Check for Rust/Cargo
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo is not installed. Please install Rust first."
        echo "Visit: https://www.rust-lang.org/tools/install"
        exit 1
    fi

    # Check for Docker
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed. Please install Docker first."
        echo "Visit: https://docs.docker.com/get-docker/"
        exit 1
    fi

    # Check for Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed. Please install Docker Compose first."
        echo "Visit: https://docs.docker.com/compose/install/"
        exit 1
    fi

    print_status "All prerequisites are installed."
}

setup_environment() {
    print_status "Setting up environment configuration..."

    if [ ! -f "$ENV_FILE" ]; then
        if [ -f "$PROJECT_ROOT/.env.example" ]; then
            print_warning "No .env file found. Creating from .env.example..."
            cp "$PROJECT_ROOT/.env.example" "$ENV_FILE"
            print_warning "Please edit $ENV_FILE with your production values before continuing."
            echo "Press Enter when ready to continue..."
            read
        else
            print_error "No .env file found and no .env.example to copy from."
            echo "Please create a .env file with the following variables:"
            echo "  DB_USERNAME=<your_db_username>"
            echo "  DB_PASSWORD=<your_db_password>"
            echo "  DB_HOST=localhost"
            echo "  DB_PORT=8000"
            echo "  DB_NAMESPACE=slatehub"
            echo "  DB_NAME=main"
            echo "  SERVER_HOST=0.0.0.0  # For production, bind to all interfaces"
            echo "  SERVER_PORT=80        # Or 443 for HTTPS, or 3000 for non-privileged"
            exit 1
        fi
    fi

    # Source the .env file to check SERVER_PORT
    source "$ENV_FILE"

    # Default to port 3000 if not set
    SERVER_PORT=${SERVER_PORT:-3000}

    print_status "Environment configured. Server will run on port $SERVER_PORT"
}

build_release() {
    print_status "Building release binary..."
    cd "$SERVER_DIR"
    cargo build --release

    if [ ! -f "$BINARY_PATH" ]; then
        print_error "Failed to build release binary."
        exit 1
    fi

    print_status "Release binary built successfully at: $BINARY_PATH"
}

setup_privileged_ports() {
    local port=$1

    if [ "$port" -lt 1024 ]; then
        print_status "Port $port is privileged. Setting up capabilities..."

        # Check if we have sudo access
        if ! sudo -n true 2>/dev/null; then
            print_warning "This operation requires sudo access."
        fi

        # Set capabilities
        sudo setcap 'cap_net_bind_service=+ep' "$BINARY_PATH"

        # Verify capabilities were set
        if getcap "$BINARY_PATH" | grep -q "cap_net_bind_service"; then
            print_status "Capabilities set successfully. Server can bind to port $port."
        else
            print_error "Failed to set capabilities."
            echo "You may need to run the server with sudo or use a port >= 1024"
            exit 1
        fi
    else
        print_status "Port $port is non-privileged. No special permissions needed."
    fi
}

start_dependencies() {
    print_status "Starting Docker dependencies (SurrealDB and MinIO)..."
    cd "$PROJECT_ROOT"
    docker-compose up -d

    print_status "Waiting for services to be ready..."
    sleep 5

    # Check if services are running
    if docker-compose ps | grep -q "Up"; then
        print_status "Docker services started successfully."
    else
        print_error "Failed to start Docker services."
        docker-compose logs
        exit 1
    fi
}

initialize_database() {
    print_status "Initializing database..."
    cd "$PROJECT_ROOT"

    if [ -f "db/schema.surql" ]; then
        make db-init
        print_status "Database initialized successfully."
    else
        print_warning "No schema.surql found. Skipping database initialization."
    fi
}

start_server_nohup() {
    print_status "Starting SlateHub server with nohup..."

    # Check if already running
    if [ -f "$PID_FILE" ]; then
        if kill -0 $(cat "$PID_FILE") 2>/dev/null; then
            print_error "Server is already running with PID $(cat $PID_FILE)"
            exit 1
        else
            print_warning "Stale PID file found. Removing..."
            rm -f "$PID_FILE"
        fi
    fi

    # Start the server
    cd "$SERVER_DIR"
    nohup "$BINARY_PATH" > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo $pid > "$PID_FILE"

    # Wait a moment and check if it's still running
    sleep 3
    if kill -0 $pid 2>/dev/null; then
        print_status "Server started successfully!"
        echo "  PID: $pid"
        echo "  Log file: $LOG_FILE"
        echo ""
        echo "To view logs: tail -f $LOG_FILE"
        echo "To stop server: $SCRIPT_DIR/stop-production.sh"
    else
        print_error "Server failed to start. Check logs at: $LOG_FILE"
        tail -20 "$LOG_FILE"
        rm -f "$PID_FILE"
        exit 1
    fi
}

create_systemd_service() {
    print_status "Creating systemd service configuration..."

    local service_file="/tmp/$SYSTEMD_SERVICE"

    cat > "$service_file" << EOF
[Unit]
Description=SlateHub Server
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
User=$(whoami)
Group=$(id -gn)
WorkingDirectory=$SERVER_DIR
ExecStartPre=$PROJECT_ROOT/scripts/start-dependencies.sh
ExecStart=$BINARY_PATH
Restart=always
RestartSec=10
StandardOutput=append:$LOG_FILE
StandardError=append:$LOG_FILE

# Environment variables
EnvironmentFile=$ENV_FILE
Environment="RUST_LOG=info"

# Security settings
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

    print_status "Systemd service file created at: $service_file"
    echo ""
    echo "To install as a system service, run:"
    echo "  sudo cp $service_file /etc/systemd/system/"
    echo "  sudo systemctl daemon-reload"
    echo "  sudo systemctl enable $SYSTEMD_SERVICE"
    echo "  sudo systemctl start $SYSTEMD_SERVICE"
    echo ""
    echo "Then manage with:"
    echo "  sudo systemctl status $SYSTEMD_SERVICE"
    echo "  sudo systemctl restart $SYSTEMD_SERVICE"
    echo "  sudo journalctl -u $SYSTEMD_SERVICE -f"
}

# Main deployment flow
main() {
    print_status "Starting SlateHub production deployment..."
    echo ""

    # Parse command line arguments
    USE_SYSTEMD=false
    SKIP_BUILD=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --systemd)
                USE_SYSTEMD=true
                shift
                ;;
            --skip-build)
                SKIP_BUILD=true
                shift
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --systemd     Create systemd service instead of using nohup"
                echo "  --skip-build  Skip building the release binary"
                echo "  --help        Show this help message"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    # Run deployment steps
    check_prerequisites
    setup_environment

    # Source the .env file to get SERVER_PORT
    source "$ENV_FILE"
    SERVER_PORT=${SERVER_PORT:-3000}

    if [ "$SKIP_BUILD" = false ]; then
        build_release
    else
        if [ ! -f "$BINARY_PATH" ]; then
            print_error "No binary found. Cannot skip build."
            exit 1
        fi
        print_status "Skipping build step (using existing binary)"
    fi

    setup_privileged_ports $SERVER_PORT
    start_dependencies
    initialize_database

    if [ "$USE_SYSTEMD" = true ]; then
        create_systemd_service
    else
        start_server_nohup
    fi

    echo ""
    print_status "Deployment complete!"
    echo ""
    echo "Server is running on port $SERVER_PORT"
    if [ "$SERVER_PORT" -eq 80 ]; then
        echo "Access at: http://your-server-ip/"
    elif [ "$SERVER_PORT" -eq 443 ]; then
        echo "Access at: https://your-server-ip/"
    else
        echo "Access at: http://your-server-ip:$SERVER_PORT/"
    fi
}

# Run main function
main "$@"
