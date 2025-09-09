#!/usr/bin/env bash

# SlateHub Development Script
# This script provides an enhanced development environment with auto-rebuild

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVER_DIR="$PROJECT_ROOT/server"
DB_DIR="$PROJECT_ROOT/db"

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if a command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Install cargo-watch if not present
install_cargo_watch() {
    if ! command_exists cargo-watch; then
        print_warning "cargo-watch is not installed"
        read -p "Would you like to install cargo-watch? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Installing cargo-watch..."
            cargo install cargo-watch
            print_success "cargo-watch installed successfully!"
        else
            print_error "cargo-watch is required for auto-rebuild functionality"
            exit 1
        fi
    else
        print_success "cargo-watch is already installed"
    fi
}

# Check Docker services
check_docker() {
    if ! command_exists docker; then
        print_error "Docker is not installed"
        exit 1
    fi

    if ! docker ps &> /dev/null; then
        print_error "Docker daemon is not running"
        exit 1
    fi

    print_success "Docker is running"
}

# Start Docker services
start_docker_services() {
    print_info "Starting Docker services..."
    cd "$PROJECT_ROOT"
    docker-compose up -d

    # Wait for services to be ready
    print_info "Waiting for services to be ready..."
    sleep 5

    # Check if SurrealDB is responding
    if curl -s -X GET http://localhost:8000/health &> /dev/null; then
        print_success "SurrealDB is ready"
    else
        print_warning "SurrealDB might not be ready yet"
    fi

    print_success "Docker services started"
}

# Stop Docker services
stop_docker_services() {
    print_info "Stopping Docker services..."
    cd "$PROJECT_ROOT"
    docker-compose down
    print_success "Docker services stopped"
}

# Initialize database
init_database() {
    print_info "Initializing database..."
    if [ -f "$DB_DIR/schema.surql" ]; then
        docker exec -i slatehub-surrealdb /surreal import \
            --conn http://localhost:8000 \
            --user root \
            --pass root \
            --ns slatehub \
            --db main \
            /dev/stdin < "$DB_DIR/schema.surql"
        print_success "Database schema loaded"
    else
        print_warning "schema.surql not found, skipping database initialization"
    fi
}

# Watch modes
watch_build() {
    print_info "Starting watch mode (build only)..."
    cd "$SERVER_DIR"
    cargo watch -x build -w src
}

watch_run() {
    print_info "Starting watch mode (build and run)..."
    print_info "Server will restart automatically when you save changes!"
    cd "$SERVER_DIR"
    cargo watch -x run -w src -w templates -w static
}

watch_test() {
    print_info "Starting watch mode (run tests)..."
    cd "$SERVER_DIR"
    cargo watch -x test -w src
}

watch_check() {
    print_info "Starting watch mode (check only - fast feedback)..."
    cd "$SERVER_DIR"
    cargo watch -x check -w src
}

watch_full() {
    print_info "Starting full watch mode..."
    print_info "Watching: Rust code, templates, static files, and database schema"
    cd "$SERVER_DIR"
    cargo watch -x run -w src -w templates -w static -w ../db/schema.surql
}

# Custom watch with specific features
watch_custom() {
    local watch_cmd="cargo watch"
    local paths=""

    print_info "Custom watch mode configuration:"

    # Command to run
    echo "What command should run on changes?"
    echo "  1) build    - Compile only"
    echo "  2) run      - Compile and run"
    echo "  3) test     - Run tests"
    echo "  4) check    - Type check only (fastest)"
    echo "  5) clippy   - Run linter"
    read -p "Choose [1-5]: " cmd_choice

    case $cmd_choice in
        1) watch_cmd="$watch_cmd -x build" ;;
        2) watch_cmd="$watch_cmd -x run" ;;
        3) watch_cmd="$watch_cmd -x test" ;;
        4) watch_cmd="$watch_cmd -x check" ;;
        5) watch_cmd="$watch_cmd -x clippy" ;;
        *) watch_cmd="$watch_cmd -x run" ;;
    esac

    # Paths to watch
    echo "What should be watched?"
    read -p "Watch Rust source? (y/n) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] && paths="$paths -w src"

    read -p "Watch templates? (y/n) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] && paths="$paths -w templates"

    read -p "Watch static files? (y/n) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] && paths="$paths -w static"

    read -p "Watch database schema? (y/n) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] && paths="$paths -w ../db/schema.surql"

    print_info "Running: $watch_cmd $paths"
    cd "$SERVER_DIR"
    eval "$watch_cmd $paths"
}

# Show menu
show_menu() {
    echo ""
    echo "========================================="
    echo "     SlateHub Development Environment    "
    echo "========================================="
    echo ""
    echo "Quick Start:"
    echo "  1) Full Development (Docker + Watch)"
    echo "  2) Watch Only (assumes Docker running)"
    echo ""
    echo "Watch Modes:"
    echo "  3) Watch & Run (auto-restart)"
    echo "  4) Watch & Build (compile only)"
    echo "  5) Watch & Test"
    echo "  6) Watch & Check (fast feedback)"
    echo "  7) Custom Watch Mode"
    echo ""
    echo "Services:"
    echo "  8) Start Docker Services"
    echo "  9) Stop Docker Services"
    echo "  10) Initialize Database"
    echo ""
    echo "  0) Exit"
    echo ""
}

# Main menu loop
main_menu() {
    while true; do
        show_menu
        read -p "Choose an option: " choice

        case $choice in
            1)
                install_cargo_watch
                start_docker_services
                init_database
                watch_run
                ;;
            2)
                install_cargo_watch
                watch_run
                ;;
            3)
                watch_run
                ;;
            4)
                watch_build
                ;;
            5)
                watch_test
                ;;
            6)
                watch_check
                ;;
            7)
                watch_custom
                ;;
            8)
                start_docker_services
                ;;
            9)
                stop_docker_services
                ;;
            10)
                init_database
                ;;
            0)
                print_info "Goodbye!"
                exit 0
                ;;
            *)
                print_error "Invalid option"
                ;;
        esac

        # After command completes, wait for user
        if [ "$choice" != "0" ]; then
            echo ""
            read -p "Press Enter to continue..."
        fi
    done
}

# Parse command line arguments
case "${1:-}" in
    quick|dev)
        install_cargo_watch
        check_docker
        start_docker_services
        watch_run
        ;;
    watch)
        install_cargo_watch
        watch_run
        ;;
    build)
        install_cargo_watch
        watch_build
        ;;
    test)
        install_cargo_watch
        watch_test
        ;;
    check)
        install_cargo_watch
        watch_check
        ;;
    full)
        install_cargo_watch
        check_docker
        start_docker_services
        init_database
        watch_full
        ;;
    menu|"")
        main_menu
        ;;
    help)
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  quick, dev  - Quick start (Docker + watch)"
        echo "  watch       - Start watch mode (run)"
        echo "  build       - Start watch mode (build only)"
        echo "  test        - Start watch mode (tests)"
        echo "  check       - Start watch mode (type check)"
        echo "  full        - Full development mode"
        echo "  menu        - Interactive menu (default)"
        echo "  help        - Show this help"
        ;;
    *)
        print_error "Unknown command: $1"
        echo "Run '$0 help' for usage"
        exit 1
        ;;
esac
