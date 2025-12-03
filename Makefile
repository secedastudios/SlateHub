.PHONY: help start stop services-start services-stop server-start server-stop dev dev-start dev-stop logs logs-services logs-server build clean shell check-env db-init dirs wait-db

# Default target
all: help

# Load environment variables if .env exists
ifneq (,$(wildcard .env))
    include .env
    export
endif

# Default DB credentials
DB_USER ?= root
DB_PASS ?= root

# Get the current user ID to avoid permission issues with bind mounts
export UID = $(shell id -u)

help:
	@echo "SlateHub Development Commands"
	@echo "============================="
	@echo ""
	@echo "Quick Start:"
	@echo "  make start        - Start all Docker containers (services + server)"
	@echo "  make stop         - Stop all Docker containers"
	@echo "  make dev-start    - Start services + run server locally (instant CSS updates!)"
	@echo ""
	@echo "Services (MinIO + SurrealDB):"
	@echo "  make services-start - Start MinIO and SurrealDB containers"
	@echo "  make services-stop  - Stop MinIO and SurrealDB containers"
	@echo "  make logs-services  - View logs for services"
	@echo ""
	@echo "Server - Docker Mode:"
	@echo "  make server-start   - Build and run server in Docker container"
	@echo "  make server-stop    - Stop Docker server container"
	@echo "  make logs-server    - View server logs (Docker)"
	@echo "  make build          - Rebuild server Docker image"
	@echo ""
	@echo "Server - Dev Mode (local filesystem):"
	@echo "  make dev-start      - Run server locally with cargo"
	@echo "  make dev-stop       - Stop local dev server (Ctrl+C)"
	@echo "  make dev            - Alias for dev-start"
	@echo "                        (CSS/static changes are instant!)"
	@echo ""
	@echo "Database:"
	@echo "  make db-init        - Initialize database schema"
	@echo ""
	@echo "Utilities:"
	@echo "  make shell          - Open shell in server container"
	@echo "  make clean          - Stop all services and remove data"
	@echo "  make logs           - View all logs"

check-env:
	@if [ ! -f .env ]; then \
		echo "⚠️  No .env file found!"; \
		echo "📝 Creating .env from .env.example..."; \
		cp .env.example .env; \
		echo "⚠️  Please edit .env with your configuration!"; \
	fi

dirs:
	@mkdir -p db/data db/files

wait-db:
	@echo "Waiting for SurrealDB to start..."
	@sleep 5
	@echo "✅ SurrealDB should be ready."

# ============================================================================
# Quick Start Commands
# ============================================================================

start: services-start server-start
	@echo "✅ All Docker containers started!"
	@echo "   Access at: http://localhost:${SERVER_PORT:-3000}"

stop:
	@echo "🛑 Stopping all Docker containers..."
	@docker-compose down
	@echo "✅ All services stopped."

# ============================================================================
# Services Management (MinIO + SurrealDB)
# ============================================================================

services-start: check-env dirs
	@echo "🚀 Starting services (MinIO + SurrealDB)..."
	@docker-compose up -d surrealdb minio
	@$(MAKE) wait-db
	@echo "✅ Services started:"
	@echo "   SurrealDB: http://localhost:8000"
	@echo "   MinIO Console: http://localhost:9001"
	@echo "   MinIO API: http://localhost:9000"

services-stop:
	@echo "🛑 Stopping services..."
	@docker-compose stop surrealdb minio
	@echo "✅ Services stopped."

logs-services:
	@docker-compose logs -f surrealdb minio

# ============================================================================
# Server - Docker Mode (Production-like)
# ============================================================================

server-start: check-env services-start
	@echo "🐳 Building and starting SlateHub server in Docker..."
	@docker-compose up -d --build slatehub
	@echo "✅ Server running in Docker on port ${SERVER_PORT:-3000}"
	@echo "   Access at: http://localhost:${SERVER_PORT:-3000}"

server-stop:
	@echo "🛑 Stopping Docker server..."
	@docker-compose stop slatehub
	@echo "✅ Server stopped."

logs-server:
	@docker-compose logs -f slatehub

build:
	@echo "🔨 Building server Docker image..."
	@docker-compose build --no-cache slatehub
	@echo "✅ Build complete."

# ============================================================================
# Development Mode (Local filesystem with hot reload for static files)
# ============================================================================

dev-start: check-env
	@echo "🚀 Starting SlateHub server in DEVELOPMENT mode..."
	@echo ""
	@echo "   ⚠️  Make sure services are running: make services-start"
	@echo ""
	@echo "   Server will run locally with cargo:"
	@echo "   - Static files from: ./server/static/"
	@echo "   - Templates from: ./server/templates/"
	@echo "   - CSS changes are instant - just refresh browser!"
	@echo ""
	@echo "   Server will run on port ${SERVER_PORT:-3000}"
	@echo "   Access at: http://localhost:${SERVER_PORT:-3000}"
	@echo ""
	@echo "   Press Ctrl+C to stop the server"
	@echo "=================================================="
	@cd server && cargo run

dev-stop:
	@echo "ℹ️  To stop the dev server, press Ctrl+C in the terminal where it's running"
	@echo "   Services will continue running. To stop them: make services-stop"

# ============================================================================
# Database Management
# ============================================================================

db-init: wait-db
	@echo "Initializing database schema..."
	@if [ -f db/schema.surql ]; then \
		cat db/schema.surql | docker exec -i slatehub-surrealdb /surreal import --conn http://localhost:8000 --user "$(DB_USER)" --pass "$(DB_PASS)" --ns slatehub --db main /dev/stdin; \
		echo "✅ Database initialized."; \
	else \
		echo "Warning: db/schema.surql not found. Skipping initialization."; \
	fi

# ============================================================================
# Utilities
# ============================================================================

logs:
	@docker-compose logs -f

shell:
	@if docker ps | grep -q slatehub-server; then \
		docker exec -it slatehub-server /bin/bash; \
	else \
		echo "❌ Server container not running. Start with 'make server-start' first."; \
	fi

clean:
	@echo "⚠️  WARNING: This will stop all services and DELETE all data!"
	@echo -n "Are you sure? [y/N] " && read ans && [ $${ans:-N} = y ]
	@echo "Stopping all services..."
	@docker-compose down -v
	@echo "Cleaning data directories..."
	@rm -rf db/data/* db/files/* 2>/dev/null || true
	@echo "✅ Clean complete."

# ============================================================================
# Aliases for convenience
# ============================================================================

# Shorthand aliases
dev: dev-start
services: services-start
server: server-start
