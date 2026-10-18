# SlateHub Production Deployment Guide

This guide covers deploying SlateHub to a production Ubuntu/Debian server. SlateHub can be deployed using either `nohup` for simple deployments or `systemd` for more robust production environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Deployment Methods](#deployment-methods)
  - [Method 1: Using Make Commands (Simple)](#method-1-using-make-commands-simple)
  - [Method 2: Using Deployment Script (Recommended)](#method-2-using-deployment-script-recommended)
  - [Method 3: Systemd Service (Most Robust)](#method-3-systemd-service-most-robust)
- [Configuration](#configuration)
- [Privileged Ports (80/443)](#privileged-ports-80443)
- [Managing the Server](#managing-the-server)
- [Monitoring and Logs](#monitoring-and-logs)
- [Security Considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)

## Prerequisites

1. **Ubuntu/Debian Server** (tested on Ubuntu 20.04+, Debian 11+)
2. **Rust** (latest stable version)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. **Docker & Docker Compose** for SurrealDB and MinIO
   ```bash
   # Install Docker
   curl -fsSL https://get.docker.com | bash
   
   # Install Docker Compose
   sudo apt-get update
   sudo apt-get install docker-compose-plugin
   ```
4. **Git** to clone the repository
5. **Make** for build commands
   ```bash
   sudo apt-get install build-essential
   ```

## Quick Start

The fastest way to deploy SlateHub in production:

```bash
# Clone the repository
git clone https://github.com/yourusername/slatehub.git
cd slatehub

# Copy and configure environment variables
cp .env.example .env
nano .env  # Edit with your production values

# Deploy with the production script
./scripts/deploy-production.sh
```

## Deployment Methods

### Method 1: Using Make Commands (Simple)

This method uses `nohup` to run the server in the background:

```bash
# Build the release binary
make server-build

# If using port 80 or 443, setup capabilities
make setup-prod-permissions

# Start the production server
make start-prod

# View logs
make logs-prod

# Stop the server
make stop-prod

# Restart the server
make restart-prod
```

**Pros:**
- Simple and quick
- No system configuration needed
- Easy to understand

**Cons:**
- Won't automatically restart on system reboot
- Less robust than systemd
- Manual log rotation needed

### Method 2: Using Deployment Script (Recommended)

The deployment script handles the complete setup:

```bash
# Full deployment with all checks and setup
./scripts/deploy-production.sh

# Skip build if already built
./scripts/deploy-production.sh --skip-build

# Create systemd service configuration
./scripts/deploy-production.sh --systemd

# Stop the production server
./scripts/stop-production.sh

# Stop server and Docker services
./scripts/stop-production.sh --with-docker
```

**Features:**
- Checks all prerequisites
- Sets up environment configuration
- Handles privileged port permissions
- Starts Docker dependencies
- Initializes the database
- Provides clear status messages

### Method 3: Systemd Service (Most Robust)

For production environments, systemd provides the most reliable deployment:

1. **Generate the systemd service file:**
   ```bash
   make install-systemd-service
   # or
   ./scripts/deploy-production.sh --systemd
   ```

2. **Install and enable the service:**
   ```bash
   sudo cp slatehub.service.tmp /etc/systemd/system/slatehub.service
   sudo systemctl daemon-reload
   sudo systemctl enable slatehub
   sudo systemctl start slatehub
   ```

3. **Manage the service:**
   ```bash
   # Check status
   sudo systemctl status slatehub
   
   # Stop the service
   sudo systemctl stop slatehub
   
   # Restart the service
   sudo systemctl restart slatehub
   
   # View logs
   sudo journalctl -u slatehub -f
   
   # View last 100 lines of logs
   sudo journalctl -u slatehub -n 100
   ```

**Pros:**
- Automatically starts on boot
- Automatic restart on failure
- Integrated with system logging
- Process supervision
- Clean shutdown handling

## Configuration

### Environment Variables

Create a `.env` file in the project root with these variables:

```bash
# Database Configuration
DB_USERNAME=root
DB_PASSWORD=your_secure_password
DB_HOST=localhost
DB_PORT=8000
DB_NAMESPACE=slatehub
DB_NAME=main

# Server Configuration
SERVER_HOST=0.0.0.0  # Bind to all interfaces for production
SERVER_PORT=80       # Or 443 for HTTPS, 3000 for non-privileged

# Optional: Override database URL completely
# DATABASE_URL=ws://localhost:8000/rpc

# Logging
RUST_LOG=info  # Options: trace, debug, info, warn, error

# MinIO Configuration
MINIO_ENDPOINT=http://localhost:9000
MINIO_ACCESS_KEY=slatehub
MINIO_SECRET_KEY=slatehub123
MINIO_BUCKET=slatehub-media
```

### Production Settings

For production, consider these settings:

1. **Use secure passwords** for database and MinIO
2. **Set appropriate RUST_LOG level** (info or warn for production)
3. **Configure SERVER_HOST** as 0.0.0.0 to accept external connections
4. **Use environment-specific ports** based on your infrastructure

## Privileged Ports (80/443)

If you want to run SlateHub on ports 80 (HTTP) or 443 (HTTPS), you need special permissions:

### Option 1: Using Capabilities (Recommended)

```bash
# Build the release binary first
make server-build

# Grant the binary permission to bind to privileged ports
sudo setcap 'cap_net_bind_service=+ep' server/target/release/slatehub

# Verify the capability was set
getcap server/target/release/slatehub

# Now you can run the server on port 80/443 without sudo
make start-prod
```

### Option 2: Using a Reverse Proxy (Alternative)

Run SlateHub on a non-privileged port (e.g., 3000) and use Nginx as a reverse proxy:

```nginx
server {
    listen 80;
    server_name your-domain.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Managing the Server

### Starting Services

```bash
# Start everything (Docker + Server)
make start-prod

# Start only Docker services
./scripts/start-dependencies.sh

# Start with custom environment
ENV_FILE=/path/to/custom.env make start-prod
```

### Stopping Services

```bash
# Stop only the server
make stop-prod

# Stop server and Docker services
./scripts/stop-production.sh --with-docker

# Force stop without prompts
./scripts/stop-production.sh --force
```

### Restarting

```bash
# Restart the server
make restart-prod

# For systemd service
sudo systemctl restart slatehub
```

### Database Management

> **Warning:** `make db-init` **drops the whole database** and re-imports
> `db/schema.surql`. It is for fresh installs and local development only —
> never run it against production. Schema changes reach production through
> migrations, which the server applies itself (next section).

```bash
# Initialize/reset database (DESTRUCTIVE — fresh installs only)
make db-init

# Clean database (remove all data)
make db-clean

# Reset database and MinIO storage
make db-reset

# View database logs
docker logs -f slatehub-surrealdb
```

### Schema Migrations

The server applies schema changes itself. Every `server/migrations/*.surql`
file is compiled into the binary and, on boot, each file the database has not
seen yet is run once (in file-name order, inside a transaction) and recorded in
the `schema_migration` table. A normal deploy is therefore just:

```bash
git pull
make build && make server     # or: cargo build --release + restart the service
```

Watch the boot log for `applying schema migration` / `Schema migrations
applied`. If a migration fails, the server logs the error and **exits instead
of serving traffic** — fix the migration (or the data it tripped over) and
restart; nothing from the failed file is left half-applied.

Rules, enforced by the runner at boot and by `make test`:

- **Migrations never remove data.** A file containing
  `REMOVE TABLE/FIELD/DATABASE/NAMESPACE`, `DELETE` or `UNSET` is refused unless
  it carries an explicit `-- destructive: authorized by <name>, <date> — <why>`
  line, and such a line is only ever added with Chris's sign-off. See
  `server/migrations/README.md`.
- Migrations are idempotent and are mirrored into `db/schema.surql`.
- Applied files are never edited (the ledger stores a checksum; a changed file
  logs a drift warning and is skipped) — write a new file instead.

To see what production has applied:

```bash
docker exec -i slatehub-surrealdb /surreal sql --endpoint http://localhost:8000 \
    --username root --password "$DB_PASS" --namespace slatehub --database main --pretty \
    <<< "SELECT name, applied_at FROM schema_migration ORDER BY applied_at;"
```

## Monitoring and Logs

### Log Files

- **Server logs (nohup):** `slatehub.log` in project root
- **Server logs (systemd):** `sudo journalctl -u slatehub -f`
- **Docker logs:** `docker-compose logs -f`
- **SurrealDB logs:** `docker logs -f slatehub-surrealdb`
- **MinIO logs:** `docker logs -f slatehub-minio`

### Monitoring Commands

```bash
# Check if server is running (nohup)
cat slatehub.pid && ps -p $(cat slatehub.pid)

# Check systemd service status
sudo systemctl status slatehub

# Check Docker services
docker-compose ps

# Monitor resource usage
htop  # or top

# Check port binding
sudo netstat -tulpn | grep slatehub
```

### Health Checks

```bash
# Check server health
curl http://localhost:3000/health  # Adjust port as needed

# Check SurrealDB health
curl http://localhost:8000/health

# Check MinIO health
curl http://localhost:9000/minio/health/live
```

## Security Considerations

### 1. Firewall Configuration

```bash
# Allow SSH (if not already allowed)
sudo ufw allow 22/tcp

# Allow HTTP
sudo ufw allow 80/tcp

# Allow HTTPS
sudo ufw allow 443/tcp

# Enable firewall
sudo ufw enable
```

### 2. SSL/TLS Configuration

For HTTPS, use Let's Encrypt with Certbot:

```bash
# Install Certbot
sudo apt-get update
sudo apt-get install certbot

# Get certificate (standalone mode)
sudo certbot certonly --standalone -d your-domain.com

# Or use with Nginx
sudo certbot --nginx -d your-domain.com
```

### 3. Database Security

- Change default passwords in production
- Use strong, unique passwords
- Consider network isolation for database
- Regular backups

### 4. File Permissions

```bash
# Secure the .env file
chmod 600 .env

# Ensure proper ownership
chown $(whoami):$(whoami) -R .
```

### 5. Regular Updates

```bash
# Update system packages
sudo apt-get update && sudo apt-get upgrade

# Update Rust
rustup update

# Update dependencies
cd server && cargo update
```

## Troubleshooting

### Server Won't Start

1. **Check if port is already in use:**
   ```bash
   sudo lsof -i :80  # or your configured port
   ```

2. **Verify environment variables:**
   ```bash
   source .env && env | grep -E "(DB_|SERVER_)"
   ```

3. **Check logs:**
   ```bash
   tail -100 slatehub.log
   ```

### Permission Denied on Privileged Port

```bash
# Re-apply capabilities
sudo setcap 'cap_net_bind_service=+ep' server/target/release/slatehub

# Verify
getcap server/target/release/slatehub
```

### Database Connection Issues

```bash
# Check if SurrealDB is running
docker ps | grep surrealdb

# Test connection
curl http://localhost:8000/health

# Check Docker logs
docker logs slatehub-surrealdb --tail 50
```

### High Memory/CPU Usage

```bash
# Monitor processes
htop

# Check Docker stats
docker stats

# Limit Docker resources in docker-compose.yml
```

### Systemd Service Issues

```bash
# Check service status
sudo systemctl status slatehub

# View detailed logs
sudo journalctl -u slatehub -n 100 --no-pager

# Reload service after config changes
sudo systemctl daemon-reload
sudo systemctl restart slatehub
```

## Backup and Recovery

### Database Backup

A SurrealDB export is one `.surql` file holding the schema (every `DEFINE …`,
including the `schema_migration` ledger) and all data:

```bash
make db-backup            # writes backups/slatehub_<timestamp>.surql, or by hand:

docker exec slatehub-surrealdb /surreal export \
    --endpoint http://localhost:8000 --username root --password "$DB_PASS" \
    --namespace slatehub --database main --log none /dev/stdout > backup_$(date +%Y%m%d).surql
```

`--log none` is not optional: the CLI prints its own `… exported successfully`
log line to **stdout**, and with that line in the file a later import stops at
the bare timestamp (`Parse error: … unexpected character 'T' after number
token`). Always check that a fresh backup ends with a SurrealQL statement, not
a log line.

Back up `db/files/` (object storage) at the same time — the two must match.

### Database Restore

An export carries its own `DEFINE` statements, so it must be imported into an
**empty** database — importing over the live one stops at the first
"already exists". The flow is stop → drop → import → start:

`make db-restore FILE=backups/slatehub_<timestamp>.surql` runs the drop + import
with a confirmation prompt; by hand:

```bash
make server-stop                       # or: sudo systemctl stop slatehub

# DESTRUCTIVE: replaces the live database with the backup
docker exec -i slatehub-surrealdb /surreal sql \
    --endpoint http://localhost:8000 --username root --password "$DB_PASS" \
    --namespace slatehub --database main <<< "REMOVE DATABASE main;"

docker exec -i slatehub-surrealdb /surreal import \
    --endpoint http://localhost:8000 --username root --password "$DB_PASS" \
    --namespace slatehub --database main /dev/stdin < backup_20261018.surql

make server                            # boot applies any migrations newer than the backup
```

### Restoring an older backup

- **Backups taken at or after the 2026-10-18 baseline** (schema at migration
  014 or later — i.e. everything exported from production since migration 014
  went live) restore as above. On boot the runner reads the ledger that came
  with the backup — or, for a pre-runner export, finds none and treats the
  database as the baseline — and applies every newer migration. Both paths
  were verified on SurrealDB 3.1.4 (2026-10-18): a 014 export restored into an
  empty database with all 69 tables and every row count identical to the
  source, and the next boot applied 015 and recorded it; an export taken
  after this version restored with its ledger and the server reported
  `Schema is up to date`. (An export made *without* `--log none` still
  restored every row, but the import reported a parse error on the trailing
  log line — do not rely on that.)
- **Backups older than the baseline** (schema before 014) are refused at boot
  with `the schema is older than the 014 migration baseline … missing: …`,
  because the squashed migrations are no longer embedded in the binary. Apply
  the historical files from git — oldest missing first, up to 014 — then start:

  ```bash
  git ls-tree --name-only 0fd72b8 db/migrations/              # the squashed 001–014
  git show 0fd72b8:db/migrations/014_profile_reminder_tracking.surql \
      | curl -sf -X POST http://localhost:8000/sql -u "root:$DB_PASS" \
             -H 'surreal-ns: slatehub' -H 'surreal-db: main' --data-binary @-
  ```

  The boot error names the first missing definitions, which shows where in
  the sequence the backup stopped. Those files were not all written to be
  re-runnable, so apply only the ones newer than the backup's schema.
- **An empty database** (new machine, nothing restored) is refused as well —
  run `make db-init`.
- **Rolling the application back** is safe: migrations only add definitions,
  so an older binary runs against a newer database. Only restoring an older
  backup moves the schema backwards.
- **Pin the database image.** `docker-compose.yml` runs
  `surrealdb/surrealdb:latest`; a major-version jump on the next pull can
  require a datastore upgrade step and can make exports from the previous
  version non-importable. Check what production runs
  (`docker exec slatehub-surrealdb /surreal version`) and pin that tag.

### MinIO Backup

```bash
# Backup MinIO data
tar -czf minio_backup_$(date +%Y%m%d).tar.gz db/files/

# Restore MinIO data
tar -xzf minio_backup_20240101.tar.gz
```

## Support

For issues and questions:

1. Check the [troubleshooting section](#troubleshooting)
2. Review logs for error messages
3. Open an issue on GitHub with:
   - System information (OS version, Rust version)
   - Error messages from logs
   - Steps to reproduce the issue