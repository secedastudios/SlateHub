#!/bin/bash

# SlateHub Production Stop Script
# This script stops the running production server

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PID_FILE="$PROJECT_ROOT/slatehub.pid"
LOG_FILE="$PROJECT_ROOT/slatehub.log"

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

stop_server() {
    if [ -f "$PID_FILE" ]; then
        local pid=$(cat "$PID_FILE")

        if kill -0 $pid 2>/dev/null; then
            print_status "Stopping SlateHub server (PID: $pid)..."

            # Send SIGTERM for graceful shutdown
            kill $pid

            # Wait for process to terminate (max 10 seconds)
            local count=0
            while [ $count -lt 10 ]; do
                if ! kill -0 $pid 2>/dev/null; then
                    break
                fi
                sleep 1
                count=$((count + 1))
            done

            # If still running, force kill
            if kill -0 $pid 2>/dev/null; then
                print_warning "Server did not stop gracefully. Force killing..."
                kill -9 $pid
                sleep 1
            fi

            # Verify it's stopped
            if ! kill -0 $pid 2>/dev/null; then
                print_status "Server stopped successfully."
                rm -f "$PID_FILE"
            else
                print_error "Failed to stop server."
                exit 1
            fi
        else
            print_warning "PID file exists but process is not running. Cleaning up..."
            rm -f "$PID_FILE"
        fi
    else
        print_warning "No PID file found. Server may not be running."

        # Check if there's a slatehub process running anyway
        if pgrep -f "target/release/slatehub" > /dev/null; then
            print_warning "Found running slatehub process without PID file."
            echo "Running processes:"
            pgrep -f "target/release/slatehub" -l
            echo ""
            read -p "Do you want to kill these processes? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                pkill -f "target/release/slatehub"
                print_status "Processes killed."
            fi
        fi
    fi
}

stop_docker_services() {
    print_status "Stopping Docker services..."
    cd "$PROJECT_ROOT"

    if [ -f "docker-compose.yml" ]; then
        docker-compose stop
        print_status "Docker services stopped."
    else
        print_warning "docker-compose.yml not found. Skipping Docker services."
    fi
}

check_systemd_service() {
    # Check if running as systemd service
    if systemctl is-active --quiet slatehub.service 2>/dev/null; then
        print_warning "SlateHub appears to be running as a systemd service."
        echo "To stop the systemd service, use:"
        echo "  sudo systemctl stop slatehub.service"
        echo ""
        read -p "Do you want to stop the systemd service? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            sudo systemctl stop slatehub.service
            print_status "Systemd service stopped."
        fi
        return 0
    fi
    return 1
}

# Main function
main() {
    print_status "Stopping SlateHub production server..."
    echo ""

    # Parse command line arguments
    STOP_DOCKER=false
    FORCE=false

    while [[ $# -gt 0 ]]; do
        case $1 in
            --with-docker)
                STOP_DOCKER=true
                shift
                ;;
            --force)
                FORCE=true
                shift
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --with-docker  Also stop Docker services (SurrealDB, MinIO)"
                echo "  --force        Force stop without confirmation"
                echo "  --help         Show this help message"
                echo ""
                echo "Examples:"
                echo "  $0                    # Stop only the SlateHub server"
                echo "  $0 --with-docker      # Stop server and Docker services"
                echo "  $0 --force            # Stop without confirmation prompts"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    # Check if running as systemd service first
    if ! check_systemd_service; then
        # If not systemd, stop the nohup process
        stop_server
    fi

    # Stop Docker services if requested
    if [ "$STOP_DOCKER" = true ]; then
        if [ "$FORCE" = false ]; then
            echo ""
            read -p "Also stop Docker services (SurrealDB, MinIO)? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                stop_docker_services
            fi
        else
            stop_docker_services
        fi
    fi

    # Show final status
    echo ""
    print_status "Shutdown complete!"

    # Suggest log cleanup
    if [ -f "$LOG_FILE" ]; then
        local log_size=$(du -h "$LOG_FILE" | cut -f1)
        echo ""
        echo "Log file exists at: $LOG_FILE (size: $log_size)"
        if [ "$FORCE" = false ]; then
            read -p "Do you want to remove the log file? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                rm -f "$LOG_FILE"
                print_status "Log file removed."
            fi
        fi
    fi
}

# Run main function
main "$@"
