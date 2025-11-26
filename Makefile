.PHONY: help start up stop logs build clean restart shell check-env db-init seed dirs wait-db dependencies debug-net debug-dns

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
	@echo "SlateHub Management Commands"
	@echo "============================"
	@echo "make start    - Start services and follow logs"
	@echo "make up       - Start services (detached)"
	@echo "make stop     - Stop services"
	@echo "make restart  - Restart services"
	@echo "make logs     - View logs"
	@echo "make build    - Rebuild images"
	@echo "make clean    - Stop services and remove all data"
	@echo "make shell    - Open shell in server container"
	@echo "make db-init  - (Re)Initialize database schema manually"
	@echo "make dependencies - Start only SurrealDB and MinIO"
	@echo "make debug-net - Test connectivity from server to database"
	@echo "make debug-dns - Check DNS resolution for database"

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

db-init: wait-db
	@echo "Initializing database schema..."
	@if [ -f db/schema.surql ]; then \
		cat db/schema.surql | docker-compose exec -T surrealdb /surreal import --conn http://localhost:8000 --user "$(DB_USER)" --pass "$(DB_PASS)" --ns slatehub --db main /dev/stdin; \
		echo "✅ Database initialized."; \
	else \
		echo "Warning: db/schema.surql not found. Skipping initialization."; \
	fi

dependencies: check-env dirs
	UID=$(UID) docker-compose up -d --remove-orphans surrealdb minio
	@$(MAKE) wait-db
	@echo "✅ Dependencies started (SurrealDB, MinIO)."

debug-net:
	@echo "Testing connectivity from slatehub-server to surrealdb..."
	@docker-compose run --rm --entrypoint curl slatehub -v http://surrealdb:8000/health

debug-dns:
	@echo "Checking DNS resolution for surrealdb..."
	@docker-compose run --rm --entrypoint getent slatehub hosts surrealdb

up: check-env dirs
	@# Remove potential conflicting containers to avoid port bind errors
	@docker rm -f slatehub-server slatehub-server-dev 2>/dev/null || true
	UID=$(UID) docker-compose up -d --remove-orphans
	@$(MAKE) wait-db
	@echo "✅ Services started."
	@echo "   App available at port defined in .env (default 3000)"
	@echo "   Run 'make db-init' to initialize the database schema if this is a fresh install."

stop:
	docker-compose down

logs:
	docker-compose logs -f

build:
	UID=$(UID) docker-compose build --no-cache

restart: stop up

clean:
	@echo "Stopping services and removing data..."
	docker-compose down -v
	@echo "Cleaning data directories..."
	@[ -d db/data ] && rm -rf db/data/* || true
	@[ -d db/files ] && rm -rf db/files/* || true
	@echo "Clean complete."

shell:
	docker-compose exec slatehub /bin/bash

seed: db-init
