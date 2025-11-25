.PHONY: help up down logs build clean restart shell check-env db-init seed dirs wait-db

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
	@echo "make up       - Start services (detached)"
	@echo "make down     - Stop services"
	@echo "make restart  - Restart services"
	@echo "make logs     - View logs"
	@echo "make build    - Rebuild images"
	@echo "make clean    - Stop services and remove all data"
	@echo "make shell    - Open shell in server container"
	@echo "make db-init  - (Re)Initialize database schema manually"

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
	@echo "Waiting for SurrealDB to accept connections..."
	@timeout=60; \
	until [ "$$(docker inspect -f '{{.State.Health.Status}}' slatehub-surrealdb 2>/dev/null)" = "healthy" ]; do \
		if [ $$timeout -le 0 ]; then \
			echo "Timed out waiting for SurrealDB to become healthy"; \
			docker logs slatehub-surrealdb --tail 50; \
			exit 1; \
		fi; \
		echo "Waiting for database health check... ($$timeout)"; \
		sleep 1; \
		timeout=$$((timeout - 1)); \
	done
	@echo "✅ SurrealDB is ready."

db-init:
	@echo "Initializing database schema..."
	@if [ -f db/schema.surql ]; then \
		cat db/schema.surql | docker-compose exec -T surrealdb /surreal import --conn http://localhost:8000 --user "$(DB_USER)" --pass "$(DB_PASS)" --ns slatehub --db main /dev/stdin; \
		echo "✅ Database initialized."; \
	else \
		echo "Warning: db/schema.surql not found. Skipping initialization."; \
	fi

up: check-env dirs
	UID=$(UID) docker-compose up -d
	@$(MAKE) wait-db
	@echo "✅ Services started."
	@echo "   App available at port defined in .env (default 3000)"
	@echo "   Run 'make db-init' to initialize the database schema if this is a fresh install."

down:
	docker-compose down

logs:
	docker-compose logs -f

build:
	UID=$(UID) docker-compose build

restart: down up

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
