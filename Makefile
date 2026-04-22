.PHONY: all help start stop services services-stop server server-stop dev dev-stop logs logs-services logs-server build clean purge shell check-env db-init db-seed dirs wait-db rebuild-embeddings oidc-verify

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
	@echo "  make start          - Start all Docker containers (services + server)"
	@echo "  make stop           - Stop all Docker containers"
	@echo ""
	@echo "Services (RustFS + SurrealDB):"
	@echo "  make services       - Start RustFS and SurrealDB containers"
	@echo "  make services-stop  - Stop RustFS and SurrealDB containers"
	@echo "  make logs-services  - View logs for services"
	@echo ""
	@echo "Server - Docker Mode:"
	@echo "  make server         - Build and run server in Docker container"
	@echo "  make server-stop    - Stop Docker server container"
	@echo "  make logs-server    - View server logs (Docker)"
	@echo "  make build          - Rebuild server Docker image"
	@echo ""
	@echo "Development Mode (local filesystem):"
	@echo "  make dev            - Run server locally with cargo"
	@echo "                        (CSS/static changes are instant!)"
	@echo "  make dev-stop       - Instructions for stopping dev server (Ctrl+C)"
	@echo ""
	@echo "Database:"
	@echo "  make db-init           - Initialize database schema"
	@echo "  make db-seed           - Seed database with test users"
	@echo "  make db-drop           - Drop database (delete all data)"
	@echo ""
	@echo "Search:"
	@echo "  make rebuild-embeddings - Rebuild all vector embeddings for semantic search"
	@echo ""
	@echo "OIDC / OAuth:"
	@echo "  make oidc-verify    - End-to-end OIDC test against the running server"
	@echo "                        (discovery → JWKS → authorize → token → userinfo →"
	@echo "                         refresh → revoke → introspect; see scripts/verify-oidc.sh)"
	@echo ""
	@echo "Utilities:"
	@echo "  make shell          - Open shell in server container"
	@echo "  make clean          - Stop all services and remove data"
	@echo "  make purge          - Remove all project containers, images, and volumes from Docker"
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

start: services server
	@echo "✅ All Docker containers started!"
	@echo "   Access at: http://localhost:${SERVER_PORT:-3000}"

stop:
	@echo "🛑 Stopping all Docker containers..."
	@docker-compose down
	@echo "✅ All containers stopped."

# ============================================================================
# Services Management (RustFS + SurrealDB)
# ============================================================================

services: check-env dirs
	@echo "🚀 Starting services (RustFS + SurrealDB)..."
	@docker-compose up -d surrealdb rustfs
	@$(MAKE) wait-db
	@echo "✅ Services started:"
	@echo "   SurrealDB: http://localhost:8000"
	@echo "   RustFS Console: http://localhost:9001"
	@echo "   RustFS API: http://localhost:9000"

services-stop:
	@echo "🛑 Stopping services..."
	@docker-compose stop surrealdb rustfs
	@echo "✅ Services stopped."

logs-services:
	@docker-compose logs -f surrealdb rustfs

# ============================================================================
# Server - Docker Mode (Production-like)
# ============================================================================

server: check-env services
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

dev: check-env
	@echo "🚀 Starting SlateHub server in DEVELOPMENT mode..."
	@echo ""
	@echo "   ⚠️  Make sure services are running: make services"
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
	@echo "Dropping database..."
	@docker exec -i slatehub-surrealdb /surreal sql --endpoint http://localhost:8000 --username "$(DB_USER)" --password "$(DB_PASS)" --namespace slatehub --database main --pretty <<< "REMOVE DATABASE main;" > /dev/null 2>&1 || true
	@echo "Initializing database schema..."
	@if [ -f db/schema.surql ]; then \
		cat db/schema.surql | docker exec -i slatehub-surrealdb /surreal import --endpoint http://localhost:8000 --username "$(DB_USER)" --password "$(DB_PASS)" --namespace slatehub --database main /dev/stdin; \
		echo "✅ Database dropped and re-initialized."; \
	else \
		echo "Warning: db/schema.surql not found. Skipping initialization."; \
	fi

rebuild-embeddings:
	@echo "Rebuilding all vector embeddings for semantic search..."
	@cd server && cargo run --bin rebuild-embeddings

db-seed: wait-db
	@echo "Seeding test users..."
	@docker exec -i slatehub-surrealdb /surreal sql --endpoint http://localhost:8000 --username "$(DB_USER)" --password "$(DB_PASS)" --namespace slatehub --database main --pretty <<< " \
		CREATE person SET \
			username = 'kevin', \
			email = 'kevin@example.com', \
			password = crypto::argon2::generate('pass123'), \
			name = 'Kevin Smith', \
			verification_status = 'email', \
			profile = { \
				name: 'Kevin Smith', \
				headline: 'Director', \
				location: 'Los Angeles', \
				is_public: true, \
				ethnicity: [], \
				media_other: [], \
				reels: [], \
				skills: [], \
				social_links: [], \
				unions: [], \
				languages: [], \
				education: [], \
				awards: [] \
			}; \
		CREATE person SET \
			username = 'chris', \
			email = 'chris@example.com', \
			password = crypto::argon2::generate('pass123'), \
			name = 'Chris Pacino', \
			verification_status = 'identity', \
			is_admin = true, \
			profile = { \
				name: 'Chris Pacino', \
				headline: 'Actor', \
				location: 'Berlin', \
				is_public: true, \
				ethnicity: [], \
				media_other: [], \
				reels: [], \
				skills: [], \
				social_links: [], \
				unions: [], \
				languages: [], \
				education: [], \
				awards: [] \
			}; \
		LET \$$chris = (SELECT id FROM person WHERE username = 'chris')[0].id; \
		LET \$$org_type = (SELECT id FROM organization_type WHERE name = 'Production Company')[0].id; \
		CREATE organization SET \
			name = 'Seceda', \
			slug = 'seceda', \
			type = \$$org_type, \
			description = 'Production company', \
			location = 'Berlin', \
			public = true, \
			verified = false, \
			social_links = [], \
			services = []; \
		LET \$$seceda = (SELECT id FROM organization WHERE slug = 'seceda')[0].id; \
		RELATE \$$chris->member_of->\$$seceda SET \
			role = 'owner', \
			invitation_status = 'accepted'; \
	"
	@echo "✅ Seeded users: kevin (pass123), chris (pass123, admin, verified)"
	@echo "✅ Seeded org: Seceda (owned by chris)"

db-seed-jobs: wait-db
	@echo "Seeding 100 job postings..."
	@docker exec -i slatehub-surrealdb /surreal sql --endpoint http://localhost:8000 --username "$(DB_USER)" --password "$(DB_PASS)" --namespace slatehub --database main --pretty < db/seed-jobs.surql
	@echo "✅ Seeded 100 job postings (posted by chris and Seceda)"

db-migrate: wait-db
	@if [ -z "$(MIGRATION)" ]; then \
		echo "Usage: make db-migrate MIGRATION=001_production_roles_to_array"; \
		echo "Available migrations:"; \
		ls db/migrations/*.surql 2>/dev/null | sed 's|db/migrations/||;s|\.surql||' | sed 's/^/  /'; \
	elif [ ! -f "db/migrations/$(MIGRATION).surql" ]; then \
		echo "❌ Migration not found: db/migrations/$(MIGRATION).surql"; \
		exit 1; \
	else \
		echo "Running migration: $(MIGRATION)..."; \
		RESULT=$$(curl -sf -X POST "http://localhost:8000/sql" \
			-H "Accept: application/json" \
			-H "surreal-ns: slatehub" \
			-H "surreal-db: main" \
			-u "$(DB_USER):$(DB_PASS)" \
			--data-binary @db/migrations/$(MIGRATION).surql); \
		echo "$$RESULT" | python3 -m json.tool 2>/dev/null || echo "$$RESULT"; \
		echo "✅ Migration $(MIGRATION) complete."; \
	fi

db-drop:
	@echo "⚠️  WARNING: This will delete the entire database!"
	@echo -n "Are you sure? [y/N] " && read ans && [ $${ans:-N} = y ]
	@echo "Dropping database..."
	@docker exec -i slatehub-surrealdb /surreal sql --endpoint http://localhost:8000 --username "$(DB_USER)" --password "$(DB_PASS)" --namespace slatehub --database main --pretty <<< "REMOVE DATABASE main;"
	@echo "✅ Database dropped."

# ============================================================================
# Utilities
# ============================================================================

logs:
	@docker-compose logs -f

shell:
	@if docker ps | grep -q slatehub-server; then \
		docker exec -it slatehub-server /bin/bash; \
	else \
		echo "❌ Server container not running. Start with 'make server' first."; \
	fi

clean:
	@echo "⚠️  WARNING: This will stop all services and DELETE all data!"
	@echo -n "Are you sure? [y/N] " && read ans && [ $${ans:-N} = y ]
	@echo "Stopping all services..."
	@docker-compose down -v
	@echo "Cleaning data directories..."
	@rm -rf db/data/* db/files/* 2>/dev/null || true
	@echo "✅ Clean complete."

purge:
	@echo "⚠️  WARNING: This will remove all slatehub containers, images, and volumes from Docker!"
	@echo -n "Are you sure? [y/N] " && read ans && [ $${ans:-N} = y ]
	@echo "🛑 Stopping and removing containers..."
	@docker-compose down --volumes --remove-orphans
	@echo "🗑️  Removing project images..."
	@docker images --filter "reference=slatehub*" --filter "reference=rustfs/rustfs*" --filter "reference=surrealdb/surrealdb*" -q | xargs -r docker rmi -f
	@echo "🗑️  Removing dangling build cache..."
	@docker builder prune -f --filter "label=com.docker.compose.project=slatehub" 2>/dev/null || true
	@echo "✅ Purge complete. All slatehub Docker resources have been removed."

# ============================================================================
# Testing
# ============================================================================

.PHONY: test test-services test-services-stop test-db-init test-wait-db

test-wait-db:
	@echo "Waiting for test SurrealDB..."
	@for i in 1 2 3 4 5 6 7 8 9 10; do \
		docker exec slatehub-surrealdb-test /surreal isready 2>/dev/null && break; \
		sleep 1; \
	done
	@echo "Test SurrealDB ready."

test-services:
	@echo "Starting test services..."
	@docker compose -f docker-compose.test.yml up -d
	@$(MAKE) test-wait-db

test-services-stop:
	@echo "Stopping test services..."
	@docker compose -f docker-compose.test.yml down

test-db-init: test-wait-db
	@echo "Initializing test database schema..."
	@echo "REMOVE DATABASE IF EXISTS test;" | docker exec -i slatehub-surrealdb-test /surreal sql --endpoint http://localhost:8000 --username root --password root --namespace slatehub-test > /dev/null 2>&1 || true
	@cat db/schema.surql | docker exec -i slatehub-surrealdb-test /surreal import --endpoint http://localhost:8000 --username root --password root --namespace slatehub-test --database test /dev/stdin
	@echo "Test database initialized."

test: test-services test-db-init
	@echo "Running tests..."
	@cd server && cargo test --lib --tests -- --test-threads=1; \
	EXIT_CODE=$$?; \
	cd .. && $(MAKE) test-services-stop; \
	exit $$EXIT_CODE

# End-to-end OIDC verification — assumes server + db are up locally.
# Runs the full Authorization Code + PKCE flow and asserts every endpoint.
oidc-verify:
	@scripts/verify-oidc.sh
