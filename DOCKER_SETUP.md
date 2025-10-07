# SlateHub Docker Setup Guide

## Overview

SlateHub now runs entirely in Docker containers, making it easy to develop and deploy with consistent environments. The setup includes:

- **SlateHub Server**: The main Rust application server
- **SurrealDB**: The database backend
- **MinIO**: S3-compatible object storage for files

## Prerequisites

- Docker and Docker Compose installed
- Make (optional but recommended)
- 2GB of available RAM
- 5GB of available disk space

## Quick Start

### 1. Clone and Setup

```bash
# Clone the repository
git clone <repository-url>
cd slatehub

# Create your environment configuration
cp .env.example .env

# Edit .env with your settings
# For development, the defaults should work fine
```

### 2. Start Development Environment

```bash
# Start all services in development mode
make dev

# Or if you prefer Docker Compose directly
docker-compose -f docker-compose.dev.yml up --build
```

The services will be available at:
- **Application**: http://localhost:3000 (development) or http://localhost:80 (production)
- **SurrealDB**: http://localhost:8000
- **MinIO Console**: http://localhost:9001 (user: slatehub, pass: slatehub123)

## Environment Configuration

### Using .env Files

The project uses `.env` files for configuration. Never commit `.env` to version control!

```bash
# For development
cp .env.example .env

# For production, create a secure .env with production values
cp .env.example .env.production
```

### Key Configuration Variables

#### Server Configuration
```env
SERVER_HOST=0.0.0.0
SERVER_PORT=3000  # Development: 3000, Production: 80
ENVIRONMENT=development  # or production
```

#### Database Configuration
```env
DB_HOST=surrealdb      # Use container name in Docker
DB_PORT=8000
DB_USERNAME=root        # Change in production!
DB_PASSWORD=root        # Change in production!
DB_NAMESPACE=slatehub
DB_NAME=main
```

#### S3/MinIO Configuration
```env
S3_ENDPOINT=http://minio:9000
S3_REGION=us-east-1
S3_ACCESS_KEY_ID=slatehub
S3_SECRET_ACCESS_KEY=slatehub123  # Change in production!
S3_BUCKET=slatehub
```

#### Logging
```env
RUST_LOG=info           # trace, debug, info, warn, error
LOG_FORMAT=pretty       # pretty, json, compact
```

## Docker Compose Files

### Development: `docker-compose.dev.yml`
- Includes verbose logging
- Mounts local templates for hot reload
- Exposes all ports for debugging
- MinIO console enabled

### Production: `docker-compose.prod.yml`
- Optimized logging (JSON format)
- Resource limits configured
- Health checks enabled
- MinIO console disabled for security
- Services only exposed locally (use reverse proxy)
- **Port 80 mapped by default** (container runs on 3000, mapped to host port 80)

### Base: `docker-compose.yml`
- Fallback configuration
- Used when no environment specified

## Common Commands

### Development

```bash
# Start development environment
make dev

# Start in background
make dev-detached

# View logs
make logs

# View specific service logs
make logs-server
make logs-surreal
make logs-minio

# Restart services
make restart

# Stop services
make stop

# Clean everything (WARNING: deletes all data)
make clean
```

### Building

```bash
# Build Docker image
make build

# Force rebuild (no cache)
make rebuild

# Build for production
make prod-build
```

### Database Management

```bash
# Initialize database
make db-init

# Reset database (WARNING: deletes all data)
make db-reset

# Clean MinIO storage
make minio-clean

# List MinIO contents
make minio-list
```

### Production

```bash
# Start production environment
make prod

# Deploy (starts prod with health checks)
make deploy

# Create backup
make backup

# Restore from backup
make restore BACKUP_FILE=backups/slatehub-20240101-120000.sql
```

### Testing

```bash
# Run all tests
make test

# Run unit tests only
make test-unit

# Run integration tests only
make test-integration
```

## Docker Image Details

The Rust server uses a multi-stage build for optimal size:

1. **Build Stage**: Uses `rust:slim` (latest stable) to compile the application
2. **Runtime Stage**: Uses `debian:bookworm-slim` with only runtime dependencies
3. **Security**: Runs as non-root user `slatehub` (UID 1001)

## Networking

All services communicate through the `slatehub-network` Docker bridge network:

- Services reference each other by container name (e.g., `surrealdb`, `minio`)
- No need for localhost/127.0.0.1 between containers
- Ports are exposed to host for development access

## Data Persistence

Data is stored in local directories (git-ignored):

- **Database**: `./db/data/`
- **File Storage**: `./db/files/`

For production, consider using Docker volumes instead:

```yaml
volumes:
  surrealdb-data:
    driver: local
  minio-data:
    driver: local
```

## Troubleshooting

### Services won't start

```bash
# Check service status
make status

# View detailed logs
docker-compose -f docker-compose.dev.yml logs -f

# Ensure .env file exists
make check-env
```

### Port conflicts

If ports are already in use, modify them in `.env`:

```env
SERVER_PORT=3001
```

### Database connection issues

```bash
# Verify SurrealDB is running
docker ps | grep surrealdb

# Test connection
curl http://localhost:8000/health
```

### Clean restart

```bash
# Stop everything and clean data
make clean

# Rebuild and start fresh
make rebuild
make db-init
make dev
```

## Production Deployment

### 1. Prepare Environment

```bash
# Create production .env from production template
cp .env.production.example .env.production

# Edit with secure values
vim .env.production

# Set secure passwords and secrets
# - Generate strong DB_PASSWORD
# - Generate JWT_SECRET
# - Set proper S3 credentials
# - Port 80 is set by default for production
```

**Note on Port 80**: The production configuration maps port 80 on the host to port 3000 in the container. The application always runs on port 3000 inside the container, but Docker handles the port mapping. If you need to use a different port, adjust `SERVER_PORT` in `.env.production`.

### 2. Build and Deploy

```bash
# Build production image
make prod-build

# Start production services (will use port 80)
make prod

# Or with custom env file
docker-compose -f docker-compose.prod.yml --env-file .env.production up -d

# Note: On Linux, you may need sudo for port 80
sudo docker-compose -f docker-compose.prod.yml --env-file .env.production up -d
```

**Port 80 Permissions**: On Linux systems, binding to port 80 requires elevated privileges. You have several options:
- Run Docker commands with `sudo` (simplest)
- Add your user to the docker group and configure Docker to allow privileged ports
- Use a reverse proxy (recommended for production)

### 3. Setup Reverse Proxy (Recommended)

For production with HTTPS, use Nginx or Traefik as a reverse proxy:

```nginx
server {
    listen 80;
    server_name yourdomain.com;
    
    # Redirect HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name yourdomain.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        proxy_pass http://localhost:80;  # SlateHub on port 80
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

If running SlateHub directly on port 80 without a reverse proxy, ensure your firewall allows traffic on port 80.

### 4. Enable SSL/TLS

Use Let's Encrypt with certbot or your preferred SSL solution:

```bash
certbot --nginx -d yourdomain.com
```

## Monitoring

### Health Checks

All services include health check endpoints:

```bash
# Application health
curl http://localhost:3000/health

# SurrealDB health
curl http://localhost:8000/health

# MinIO health
curl http://localhost:9000/minio/health/live
```

### Logs

For production, consider using a log aggregation service:

```bash
# Stream logs to file
docker-compose -f docker-compose.prod.yml logs -f > slatehub.log

# Use JSON format for structured logging
RUST_LOG=info LOG_FORMAT=json make prod
```

## Security Considerations

### For Production

1. **Change all default passwords** in `.env`
2. **Use strong secrets** for JWT and sessions
3. **Disable MinIO console** (already done in prod config)
4. **Use HTTPS** with proper certificates
5. **Implement rate limiting** (configured via env vars)
6. **Regular backups** (use `make backup`)
7. **Monitor logs** for suspicious activity
8. **Keep Docker images updated**

### Environment Variables

Never commit `.env` files with real credentials. Use:
- Secret management systems (Vault, AWS Secrets Manager)
- Docker secrets for Swarm mode
- Kubernetes secrets for K8s deployments

## Migration from Standalone

If migrating from the old standalone setup:

1. **Export data** from existing database
2. **Copy file storage** to `./db/files/`
3. **Update configuration** in `.env`
4. **Import data** to new containerized database
5. **Test thoroughly** before switching production

## Support

For issues or questions:
1. Check the logs: `make logs`
2. Verify configuration: `make check-env`
3. Check service status: `make status`
4. Review this documentation
5. Open an issue on GitHub