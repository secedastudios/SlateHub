.PHONY: help all start stop clean dev prod build logs status test test-unit test-integration docker-up docker-down docker-build docker-logs db-init db-reset minio-clean

# Default target
all: help

# Help target
help:
	@echo "SlateHub Development Commands"
	@echo "=============================="
	@echo ""
	@echo "Main Commands:"
	@echo "  make start          - Start all services (builds Docker image if needed)"
	@echo "  make stop           - Stop all services"
	@echo "  make restart        - Restart all services"
	@echo "  make clean          - Stop services and clean up data"
	@echo "  make logs           - Show logs for all services"
	@echo "  make status         - Show status of all services"
	@echo ""
	@echo "Development Commands:"
	@echo "  make dev            - Start services in development mode"
	@echo "  make build          - Build the Docker image"
	@echo "  make rebuild        - Force rebuild the Docker image"
	@echo "  make watch          - Watch for changes and auto-restart (requires cargo-watch)"
	@echo ""
	@echo "Production Commands:"
	@echo "  make prod           - Start services in production mode"
	@echo "  make prod-build     - Build optimized production image"
	@echo ""
	@echo "Database Commands:"
	@echo "  make db-init        - Initialize database with schema"
	@echo "  make db-reset       - Reset database (WARNING: deletes all data)"
	@echo "  make minio-clean    - Clean MinIO storage"
	@echo ""
	@echo "Testing Commands:"
	@echo "  make test           - Run all tests"
	@echo "  make test-unit      - Run unit tests only"
	@echo "  make test-integration - Run integration tests only"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-up      - Start Docker services"
	@echo "  make docker-down    - Stop Docker services"
	@echo "  make docker-logs    - Show Docker logs"
	@echo ""
	@echo "Environment Setup:"
	@echo "  make setup          - Initial project setup"
	@echo "  make check-env      - Check environment configuration"

# Main commands
start: docker-build docker-up
	@echo "✅ SlateHub is running at http://localhost:3000"
	@echo "📊 SurrealDB is available at http://localhost:8000"
	@echo "📦 MinIO console is available at http://localhost:9001"

stop: docker-down

restart: stop start

clean: docker-down
	@echo "🧹 Cleaning up data directories..."
	@rm -rf db/data/* db/files/* 2>/dev/null || true
	@echo "✅ Cleanup complete"

# Development commands
dev: check-env
	@echo "🚀 Starting SlateHub in development mode..."
	@docker-compose -f docker-compose.dev.yml up --build

dev-detached: check-env
	@echo "🚀 Starting SlateHub in development mode (detached)..."
	@docker-compose -f docker-compose.dev.yml up -d --build

# Production commands
prod: check-env prod-build
	@echo "🚀 Starting SlateHub in production mode..."
	@echo "📌 Note: Production uses port 80. You may need sudo on Linux."
	@docker-compose -f docker-compose.prod.yml up -d
	@echo "✅ SlateHub is running at http://localhost (port 80)"

prod-build:
	@echo "🏗️  Building production Docker image..."
	@docker-compose -f docker-compose.prod.yml build --no-cache

# Build commands
build:
	@echo "🏗️  Building Docker image..."
	@docker-compose -f docker-compose.dev.yml build

rebuild:
	@echo "🏗️  Rebuilding Docker image (no cache)..."
	@docker-compose -f docker-compose.dev.yml build --no-cache

# Docker commands
docker-up:
	@echo "🐳 Starting Docker services..."
	@if [ -f .env ]; then \
		docker-compose -f docker-compose.dev.yml up -d; \
	else \
		echo "⚠️  No .env file found. Using docker-compose.yml..."; \
		docker-compose up -d; \
	fi

docker-down:
	@echo "🛑 Stopping Docker services..."
	@docker-compose -f docker-compose.dev.yml down 2>/dev/null || docker-compose down

docker-build:
	@echo "🏗️  Building SlateHub Docker image..."
	@docker build -t slatehub:latest ./server

docker-logs:
	@docker-compose -f docker-compose.dev.yml logs -f || docker-compose logs -f

# Specific service logs
logs-server:
	@docker logs -f slatehub-server-dev 2>/dev/null || docker logs -f slatehub-server

logs-surreal:
	@docker logs -f slatehub-surrealdb-dev 2>/dev/null || docker logs -f slatehub-surrealdb

logs-minio:
	@docker logs -f slatehub-minio-dev 2>/dev/null || docker logs -f slatehub-minio

logs: docker-logs

# Database commands
db-init:
	@echo "🗄️  Initializing database..."
	@docker exec -it slatehub-surrealdb-dev sh -c "echo 'USE NS slatehub DB main;' | surreal sql --conn http://localhost:8000 --user root --pass root" 2>/dev/null || \
	docker exec -it slatehub-surrealdb sh -c "echo 'USE NS slatehub DB main;' | surreal sql --conn http://localhost:8000 --user root --pass root"
	@echo "✅ Database initialized"

db-reset: minio-clean
	@echo "⚠️  Resetting database (this will delete all data)..."
	@read -p "Are you sure? [y/N] " confirm && [ "$$confirm" = "y" ] || exit 1
	@docker-compose down
	@rm -rf db/data/* 2>/dev/null || true
	@docker-compose up -d surrealdb
	@sleep 3
	@$(MAKE) db-init
	@echo "✅ Database reset complete"

# MinIO/S3 commands
minio-clean:
	@echo "🧹 Cleaning MinIO storage..."
	@docker exec -it slatehub-minio-dev sh -c "mc alias set local http://localhost:9000 slatehub slatehub123 && mc rb --force local/slatehub 2>/dev/null || true" 2>/dev/null || \
	docker exec -it slatehub-minio sh -c "mc alias set local http://localhost:9000 slatehub slatehub123 && mc rb --force local/slatehub 2>/dev/null || true" || true
	@rm -rf db/files/* 2>/dev/null || true
	@echo "✅ MinIO storage cleaned"

minio-list:
	@echo "📦 MinIO buckets and objects:"
	@docker exec -it slatehub-minio-dev sh -c "mc alias set local http://localhost:9000 slatehub slatehub123 && mc ls local/" 2>/dev/null || \
	docker exec -it slatehub-minio sh -c "mc alias set local http://localhost:9000 slatehub slatehub123 && mc ls local/" || \
	echo "MinIO not running or no buckets found"

# Status command
status:
	@echo "📊 Service Status:"
	@echo "=================="
	@docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" | grep -E "slatehub|surreal|minio" || echo "No services running"

# Test commands
test-setup:
	@echo "🧪 Setting up test environment..."
	@cp server/.env.test server/.env 2>/dev/null || true
	@docker-compose -f docker-compose.test.yml up -d
	@sleep 3

test-teardown:
	@echo "🧹 Cleaning up test environment..."
	@docker-compose -f docker-compose.test.yml down -v

test: test-setup
	@echo "🧪 Running all tests..."
	@docker run --rm \
		--network slatehub-network \
		-e DATABASE_URL=ws://surrealdb:8000 \
		-e DB_USERNAME=root \
		-e DB_PASSWORD=root \
		-v $(PWD)/server:/app \
		-w /app \
		rust:slim \
		cargo test -- --nocapture
	@$(MAKE) test-teardown

test-unit: test-setup
	@echo "🧪 Running unit tests..."
	@docker run --rm \
		--network slatehub-network \
		-v $(PWD)/server:/app \
		-w /app \
		rust:slim \
		cargo test --lib -- --nocapture
	@$(MAKE) test-teardown

test-integration: test-setup
	@echo "🧪 Running integration tests..."
	@docker run --rm \
		--network slatehub-network \
		-e DATABASE_URL=ws://surrealdb:8000 \
		-e DB_USERNAME=root \
		-e DB_PASSWORD=root \
		-v $(PWD)/server:/app \
		-w /app \
		rust:slim \
		cargo test --test '*' -- --nocapture
	@$(MAKE) test-teardown

# Development tools
watch:
	@echo "👁️  Watching for changes..."
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cd server && cargo watch -x "check" -x "test" -x "run"; \
	else \
		echo "⚠️  cargo-watch not installed. Install with: cargo install cargo-watch"; \
		exit 1; \
	fi

# Setup commands
setup: check-env
	@echo "🔧 Setting up SlateHub..."
	@if [ ! -f .env ]; then \
		echo "📝 Creating .env file from .env.example..."; \
		cp .env.example .env; \
		echo "⚠️  Please edit .env file with your configuration"; \
	fi
	@mkdir -p db/data db/files
	@echo "✅ Setup complete. Run 'make start' to begin."

check-env:
	@if [ ! -f .env ]; then \
		echo "⚠️  No .env file found!"; \
		echo "📝 Creating .env from .env.example..."; \
		cp .env.example .env; \
		echo ""; \
		echo "⚠️  IMPORTANT: Edit .env file with your configuration before continuing!"; \
		echo ""; \
		exit 1; \
	fi
	@echo "✅ Environment file found"

# Install development dependencies
install-deps:
	@echo "📦 Installing development dependencies..."
	@if command -v cargo >/dev/null 2>&1; then \
		cargo install cargo-watch; \
		cargo install cargo-edit; \
		echo "✅ Rust development tools installed"; \
	else \
		echo "⚠️  Rust not installed. Please install Rust first."; \
	fi

# Quick start for new developers
quick-start: setup docker-build db-init dev
	@echo "✅ SlateHub is ready for development!"

# Production deployment helper
deploy: prod
	@echo "✅ SlateHub deployed in production mode on port 80"
	@echo "📊 Running health checks..."
	@sleep 5
	@curl -f http://localhost/api/health || echo "⚠️  Health check failed"

# Backup commands
backup:
	@echo "💾 Creating backup..."
	@mkdir -p backups
	@docker exec slatehub-surrealdb sh -c "surreal export --conn http://localhost:8000 --user root --pass root --ns slatehub --db main" > backups/slatehub-$(shell date +%Y%m%d-%H%M%S).sql
	@tar -czf backups/minio-$(shell date +%Y%m%d-%H%M%S).tar.gz db/files/
	@echo "✅ Backup complete"

restore:
	@echo "📥 Restoring from backup..."
	@if [ -z "$(BACKUP_FILE)" ]; then \
		echo "⚠️  Please specify BACKUP_FILE=path/to/backup.sql"; \
		exit 1; \
	fi
	@docker exec -i slatehub-surrealdb sh -c "surreal import --conn http://localhost:8000 --user root --pass root --ns slatehub --db main" < $(BACKUP_FILE)
	@echo "✅ Restore complete"

# Version and info
version:
	@echo "SlateHub Version Information:"
	@echo "============================="
	@cd server && cargo version
	@docker --version
	@docker-compose --version

info: version status
	@echo ""
	@echo "Environment: $(shell if [ -f .env ]; then echo "Configured"; else echo "Not configured"; fi)"
	@echo "Data directory: ./db"
	@echo "Development URL: http://localhost:3000"
	@echo "Production URL: http://localhost (port 80)"
