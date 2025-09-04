.PHONY: help all start stop clean db-start db-stop db-init db-drop db-reset server-run server-build docker-up docker-down docker-restart docker-logs

# Default target - show help
help:
	@echo "SlateHub Makefile Commands:"
	@echo ""
	@echo "Main Commands:"
	@echo "  make start          - Start everything (Docker services + server, assumes DB is initialized)"
	@echo "  make stop           - Stop everything"
	@echo "  make clean          - Stop everything and clean data files"
	@echo ""
	@echo "Docker Commands:"
	@echo "  make docker-up      - Start Docker services (SurrealDB + MinIO)"
	@echo "  make docker-down    - Stop Docker services"
	@echo "  make docker-restart - Restart Docker services"
	@echo "  make docker-logs    - Show Docker container logs"
	@echo ""
	@echo "Database Commands:"
	@echo "  make db-start       - Start SurrealDB container"
	@echo "  make db-stop        - Stop SurrealDB container"
	@echo "  make db-init        - Clean and initialize database with schema from db/schema.surql"
	@echo "  make db-clean       - Remove all database content (tables, functions, etc.)"
	@echo "  make db-drop        - Drop the entire database"
	@echo "  make db-reset       - Alias for db-init (clean and reinitialize)"
	@echo ""
	@echo "Server Commands:"
	@echo "  make server-run     - Run the Rust server (cargo run)"
	@echo "  make server-build   - Build the Rust server (cargo build --release)"

# Main combined commands
start: docker-up server-run

stop: docker-down

all: start

clean: docker-down
	@echo "Cleaning database data..."
	@rm -rf db/data/*
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

db-drop:
	@echo "Dropping database..."
	@echo "REMOVE DATABASE main;" | docker exec -i slatehub-surrealdb /surreal sql \
		--conn http://localhost:8000 \
		--user root \
		--pass root \
		--ns slatehub \
		--db main || true
	@echo "Database dropped!"

db-reset: db-init
	@echo "Database reset complete!"

# Server commands
server-run:
	@echo "Starting SlateHub server..."
	@cd server && cargo run

server-build:
	@echo "Building SlateHub server..."
	@cd server && cargo build --release
	@echo "Build complete! Binary at: server/target/release/slatehub"

# Development helpers
dev: docker-up
	@echo "Starting in development mode..."
	@cd server && cargo watch -x run

logs-surreal:
	@docker logs -f slatehub-surrealdb

logs-minio:
	@docker logs -f slatehub-minio

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
