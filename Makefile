.PHONY: help all start stop clean db-start db-stop db-init db-drop db-reset server-run server-build docker-up docker-down docker-restart docker-logs minio-clean minio-list test test-setup test-teardown test-watch test-integration test-unit test-clean start-prod stop-prod restart-prod logs-prod setup-prod-permissions install-systemd-service

# Default target - show help
help:
	@echo "SlateHub Makefile Commands:"
	@echo ""
	@echo "Main Commands:"
	@echo "  make start          - Start everything (Docker services + server, assumes DB is initialized)"
	@echo "  make stop           - Stop everything"
	@echo "  make clean          - Stop everything and clean data files"
	@echo ""
	@echo "Development Commands (with auto-rebuild):"
	@echo "  make dev            - Start Docker + watch server (auto-rebuild on changes)"
	@echo "  make watch          - Watch and auto-rebuild server only"
	@echo "  make watch-run      - Watch, rebuild and restart server on changes"
	@echo "  make watch-test     - Watch and run tests on changes"
	@echo "  make install-watch  - Install cargo-watch if not already installed"
	@echo ""
	@echo "Test Commands:"
	@echo "  make test           - Run all tests with setup/teardown"
	@echo "  make test-setup     - Start test environment (SurrealDB + MinIO on test ports)"
	@echo "  make test-teardown  - Stop test environment and clean test data"
	@echo "  make test-watch     - Watch and run tests on changes"
	@echo "  make test-integration - Run integration tests only"
	@echo "  make test-unit      - Run unit tests only"
	@echo "  make test-clean     - Clean all test data and containers"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-up      - Start Docker services (SurrealDB + MinIO)"
	@echo "  make docker-down    - Stop Docker services"
	@echo "  make docker-restart - Restart Docker services"
	@echo "  make docker-logs    - Show Docker container logs"
	@echo ""
	@echo "MinIO Commands:"
	@echo "  make minio-clean    - Delete all files from MinIO storage"
	@echo "  make minio-list     - List all files in MinIO storage"
	@echo ""
	@echo "Database Commands:"
	@echo "  make db-start       - Start SurrealDB container"
	@echo "  make db-stop        - Stop SurrealDB container"
	@echo "  make db-init        - Clean and initialize database with schema from db/schema.surql"
	@echo "  make db-clean       - Remove all database content (tables, functions, etc.)"
	@echo "  make db-drop        - Drop the entire database and clean MinIO files"
	@echo "  make db-reset       - Alias for db-init (clean and reinitialize)"
	@echo ""
	@echo "Server Commands:"
	@echo "  make server-run     - Run the Rust server (cargo run)"
	@echo "  make server-build   - Build the Rust server (cargo build --release)"
	@echo ""
	@echo "Production Commands:"
	@echo "  make start-prod     - Start server in production (background with nohup)"
	@echo "  make stop-prod      - Stop production server"
	@echo "  make restart-prod   - Restart production server"
	@echo "  make logs-prod      - View production server logs"
	@echo "  make setup-prod-permissions - Setup capabilities for privileged ports (requires sudo)"
	@echo "  make install-systemd-service - Install systemd service (Ubuntu/Debian)"

# Main combined commands
start: docker-up server-run

stop: docker-down

all: start

clean: docker-down
	@echo "Cleaning database data..."
	@rm -rf db/data/*
	@echo "Cleaning MinIO files..."
	@rm -rf db/files/*
	@echo "Clean complete!"

# Docker commands
docker-up:
	@echo "Starting Docker services..."
	@docker-compose up -d
	@echo "Waiting for services to be ready..."
	@sleep 5
	@echo "Docker services started!"

docker-down:
	@echo "Stopping Docker services..."
	@docker-compose down
	@echo "Docker services stopped!"

docker-restart: docker-down docker-up

docker-logs:
	@docker-compose logs -f

# Database specific commands
db-start:
	@echo "Starting SurrealDB..."
	@docker-compose up -d surrealdb
	@echo "Waiting for SurrealDB to be ready..."
	@sleep 3
	@echo "SurrealDB started!"

db-stop:
	@echo "Stopping SurrealDB..."
	@docker-compose stop surrealdb
	@echo "SurrealDB stopped!"

db-clean:
	@echo "Cleaning database (removing all tables, functions, etc.)..."
	@echo "REMOVE DATABASE main; DEFINE DATABASE main;" | docker exec -i slatehub-surrealdb /surreal sql \
		--conn http://localhost:8000 \
		--user root \
		--pass root \
		--ns slatehub || true
	@echo "Database cleaned!"

db-init: db-clean
	@echo "Initializing database schema..."
	@if [ -f db/schema.surql ]; then \
		docker exec -i slatehub-surrealdb /surreal import \
			--conn http://localhost:8000 \
			--user root \
			--pass root \
			--ns slatehub \
			--db main \
			/dev/stdin < db/schema.surql && \
		echo "Database schema loaded successfully!"; \
	else \
		echo "Warning: db/schema.surql not found. Skipping schema initialization."; \
	fi

db-drop: minio-clean
	@echo "Dropping database..."
	@echo "REMOVE DATABASE main;" | docker exec -i slatehub-surrealdb /surreal sql \
		--conn http://localhost:8000 \
		--user root \
		--pass root \
		--ns slatehub \
		--db main || true
	@echo "Database dropped!"

db-reset: minio-clean db-init
	@echo "Database and MinIO storage reset complete!"

# Server commands
server-run:
	@echo "Starting SlateHub server..."
	@cd server && cargo run

server-build:
	@echo "Building SlateHub server..."
	@cd server && cargo build --release
	@echo "Build complete! Binary at: server/target/release/slatehub"

# Production commands
start-prod: server-build
	@echo "Starting SlateHub server in production mode..."
	@if [ ! -f server/target/release/slatehub ]; then \
		echo "Error: Production binary not found. Run 'make server-build' first."; \
		exit 1; \
	fi
	@if [ -f slatehub.pid ]; then \
		echo "Warning: slatehub.pid file exists. Server may already be running."; \
		echo "Run 'make stop-prod' first if you want to restart."; \
		exit 1; \
	fi
	@echo "Starting server with nohup..."
	@cd server && nohup ./target/release/slatehub > ../slatehub.log 2>&1 & echo $$! > ../slatehub.pid
	@sleep 2
	@if [ -f slatehub.pid ] && kill -0 $$(cat slatehub.pid) 2>/dev/null; then \
		echo "Server started successfully! PID: $$(cat slatehub.pid)"; \
		echo "Logs: tail -f slatehub.log"; \
	else \
		echo "Failed to start server. Check slatehub.log for details."; \
		rm -f slatehub.pid; \
		exit 1; \
	fi

stop-prod:
	@if [ -f slatehub.pid ]; then \
		echo "Stopping SlateHub server (PID: $$(cat slatehub.pid))..."; \
		kill $$(cat slatehub.pid) 2>/dev/null || true; \
		rm -f slatehub.pid; \
		echo "Server stopped."; \
	else \
		echo "No slatehub.pid file found. Server may not be running."; \
	fi

restart-prod: stop-prod start-prod

logs-prod:
	@if [ -f slatehub.log ]; then \
		tail -f slatehub.log; \
	else \
		echo "No log file found. Server may not have been started."; \
	fi

# Setup capabilities for binding to privileged ports (80, 443)
setup-prod-permissions: server-build
	@echo "Setting up capabilities for privileged port binding..."
	@echo "This requires sudo access."
	@sudo setcap 'cap_net_bind_service=+ep' server/target/release/slatehub
	@echo "Capabilities set. The server can now bind to ports below 1024."
	@echo "You can verify with: getcap server/target/release/slatehub"

# Install as systemd service (for Ubuntu/Debian systems)
install-systemd-service:
	@echo "Creating systemd service file..."
	@if [ ! -f server/target/release/slatehub ]; then \
		echo "Error: Production binary not found. Run 'make server-build' first."; \
		exit 1; \
	fi
	@echo "Creating slatehub.service file..."
	@echo "[Unit]" > slatehub.service.tmp
	@echo "Description=SlateHub Server" >> slatehub.service.tmp
	@echo "After=network.target" >> slatehub.service.tmp
	@echo "" >> slatehub.service.tmp
	@echo "[Service]" >> slatehub.service.tmp
	@echo "Type=simple" >> slatehub.service.tmp
	@echo "User=$$(whoami)" >> slatehub.service.tmp
	@echo "WorkingDirectory=$$(pwd)/server" >> slatehub.service.tmp
	@echo "ExecStart=$$(pwd)/server/target/release/slatehub" >> slatehub.service.tmp
	@echo "Restart=always" >> slatehub.service.tmp
	@echo "RestartSec=10" >> slatehub.service.tmp
	@echo "StandardOutput=append:$$(pwd)/slatehub.log" >> slatehub.service.tmp
	@echo "StandardError=append:$$(pwd)/slatehub.log" >> slatehub.service.tmp
	@echo "Environment=\"RUST_LOG=info\"" >> slatehub.service.tmp
	@echo "" >> slatehub.service.tmp
	@echo "[Install]" >> slatehub.service.tmp
	@echo "WantedBy=multi-user.target" >> slatehub.service.tmp
	@echo ""
	@echo "Systemd service file created: slatehub.service.tmp"
	@echo ""
	@echo "To install, run:"
	@echo "  sudo cp slatehub.service.tmp /etc/systemd/system/slatehub.service"
	@echo "  sudo systemctl daemon-reload"
	@echo "  sudo systemctl enable slatehub"
	@echo "  sudo systemctl start slatehub"
	@echo ""
	@echo "Then you can manage with:"
	@echo "  sudo systemctl status slatehub"
	@echo "  sudo systemctl stop slatehub"
	@echo "  sudo systemctl restart slatehub"
	@echo "  sudo journalctl -u slatehub -f"

# Development helpers with auto-rebuild
dev: docker-up
	@echo "Starting in development mode with auto-rebuild..."
	@echo "Server will restart automatically when you save changes!"
	@cd server && cargo watch -x run -w src -w templates -w static

# Watch commands for development
watch:
	@echo "Watching for changes (build only)..."
	@cd server && cargo watch -x build -w src

watch-run:
	@echo "Watching for changes (build and run)..."
	@echo "Server will restart automatically when you save changes!"
	@cd server && cargo watch -x run -w src -w templates -w static

watch-test:
	@echo "Watching for changes (run tests)..."
	@cd server && cargo watch -x test -w src

watch-check:
	@echo "Watching for changes (check only - fast feedback)..."
	@cd server && cargo watch -x check -w src

# Watch both Rust code and database schema
watch-full: docker-up
	@echo "Starting full development mode with auto-rebuild..."
	@echo "Watching Rust code, templates, and database schema..."
	@cd server && cargo watch -x run -w src -w templates -w static -w ../db/schema.surql

# Install cargo-watch if not already installed
install-watch:
	@echo "Checking if cargo-watch is installed..."
	@if ! command -v cargo-watch &> /dev/null; then \
		echo "Installing cargo-watch..."; \
		cargo install cargo-watch; \
		echo "cargo-watch installed successfully!"; \
	else \
		echo "cargo-watch is already installed!"; \
	fi

logs-surreal:
	@docker logs -f slatehub-surrealdb

logs-minio:
	@docker logs -f slatehub-minio

# MinIO commands
minio-clean:
	@echo "Cleaning all files from MinIO..."
	@if docker ps | grep -q slatehub-minio; then \
		docker exec slatehub-minio sh -c 'mc alias set local http://localhost:9000 slatehub slatehub123 2>/dev/null || true && \
			mc rm -r --force local/slatehub-media/ 2>/dev/null || true && \
			mc mb local/slatehub-media 2>/dev/null || true' && \
		echo "MinIO storage cleaned!"; \
	else \
		echo "MinIO container not running, cleaning local files..."; \
		rm -rf db/files/* 2>/dev/null || true; \
		echo "Local MinIO files cleaned!"; \
	fi

minio-list:
	@echo "Listing MinIO files..."
	@if docker ps | grep -q slatehub-minio; then \
		docker exec slatehub-minio sh -c 'mc alias set local http://localhost:9000 slatehub slatehub123 2>/dev/null || true && \
			mc ls -r local/slatehub-media/ 2>/dev/null || echo "No files found or bucket does not exist"'; \
	else \
		echo "MinIO container not running"; \
	fi

# Status check
status:
	@echo "Checking service status..."
	@echo ""
	@echo "Docker containers:"
	@docker-compose ps
	@echo ""
	@echo "SurrealDB connection test:"
	@curl -s -X GET http://localhost:8000/health || echo "SurrealDB not responding"
	@echo ""
	@echo "MinIO Console: http://localhost:9001"
	@echo "  Username: slatehub"
	@echo "  Password: slatehub123"

# Quick development start with auto-rebuild
quick-start: install-watch docker-up db-init dev

# Development with existing database
quick-dev: install-watch dev

# Test environment setup
test-setup:
	@echo "Starting test environment..."
	@echo "Creating test data directories..."
	@mkdir -p db/test-data db/test-files
	@echo "Starting test Docker services..."
	@docker-compose -f docker-compose.test.yml up -d
	@echo "Waiting for test services to be ready..."
	@sleep 5
	@echo "Test environment ready!"
	@echo "  SurrealDB Test: http://localhost:8100"
	@echo "  MinIO Test API: http://localhost:9100"
	@echo "  MinIO Test Console: http://localhost:9101"

# Test environment teardown
test-teardown:
	@echo "Stopping test environment..."
	@docker-compose -f docker-compose.test.yml down
	@echo "Test environment stopped!"

# Clean test data
test-clean: test-teardown
	@echo "Cleaning test data..."
	@rm -rf db/test-data/* db/test-files/* 2>/dev/null || true
	@echo "Removing test containers and volumes..."
	@docker-compose -f docker-compose.test.yml down -v
	@echo "Test environment cleaned!"

# Run all tests with automatic setup and teardown
test: test-setup
	@echo "Running all tests..."
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo test --all -- --test-threads=1 || (make test-teardown && exit 1)
	@make test-teardown
	@echo "All tests completed!"

# Run unit tests only
test-unit: test-setup
	@echo "Running unit tests..."
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo test --lib -- --test-threads=1 || (make test-teardown && exit 1)
	@make test-teardown
	@echo "Unit tests completed!"

# Run integration tests only
test-integration: test-setup
	@echo "Running integration tests..."
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo test --test '*' -- --test-threads=1 || (make test-teardown && exit 1)
	@make test-teardown
	@echo "Integration tests completed!"

# Watch tests with automatic rerun
test-watch: test-setup
	@echo "Watching tests (requires test environment to be running)..."
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo watch -x 'test -- --test-threads=1' -w src -w tests

# Run specific test file
test-file: test-setup
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make test-file FILE=test_name"; \
		exit 1; \
	fi
	@echo "Running test file: $(FILE)..."
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo test $(FILE) -- --test-threads=1 || (make test-teardown && exit 1)
	@make test-teardown

# Check test environment status
test-status:
	@echo "Checking test environment status..."
	@echo ""
	@echo "Test Docker containers:"
	@docker-compose -f docker-compose.test.yml ps
	@echo ""
	@echo "Test SurrealDB connection:"
	@curl -s -X GET http://localhost:8100/health || echo "Test SurrealDB not responding"
	@echo ""
	@echo "Test MinIO Console: http://localhost:9101"
	@echo "  Username: slatehub-test"
	@echo "  Password: slatehub-test123"

# Run tests with coverage (requires cargo-tarpaulin)
test-coverage: test-setup
	@echo "Running tests with coverage..."
	@if ! command -v cargo-tarpaulin &> /dev/null; then \
		echo "Installing cargo-tarpaulin..."; \
		cargo install cargo-tarpaulin; \
	fi
	@cd server && \
		DATABASE_URL=ws://localhost:8100/rpc \
		DATABASE_USER=root \
		DATABASE_PASS=root \
		DATABASE_NS=slatehub-test \
		DATABASE_DB=test \
		MINIO_ENDPOINT=http://localhost:9100 \
		MINIO_ACCESS_KEY=slatehub-test \
		MINIO_SECRET_KEY=slatehub-test123 \
		MINIO_BUCKET=slatehub-test-media \
		cargo tarpaulin --out Html --output-dir ../target/coverage || (make test-teardown && exit 1)
	@make test-teardown
	@echo "Coverage report generated at: target/coverage/tarpaulin-report.html"
