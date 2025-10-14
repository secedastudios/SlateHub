# SlateHub Production Setup Guide

This guide covers deploying SlateHub in production with Docker, including running on port 80.

## Prerequisites

- Docker and Docker Compose installed
- Linux server (Ubuntu/Debian recommended) or cloud instance
- At least 2GB RAM and 10GB disk space
- Domain name (optional but recommended)
- SSL certificate (for HTTPS)

## Quick Start

### 1. Clone the Repository

```bash
git clone <repository-url>
cd slatehub
```

### 2. Configure Environment

```bash
# Create production environment file
cp .env.production.example .env

# Edit with your production values
nano .env

# At minimum, you MUST change these values:
# - DB_USERNAME and DB_PASSWORD
# - S3_ACCESS_KEY_ID and S3_SECRET_ACCESS_KEY
# - JWT_SECRET and SESSION_SECRET
```

### 3. Deploy

```bash
# Option 1: Use the deployment script (recommended)
./deploy-production.sh

# Option 2: Manual deployment
make prod

# Option 3: Docker Compose directly
docker-compose -f docker-compose.prod.yml up -d
```

## Port 80 Configuration

### Default Setup

By default, the production configuration maps port 80 on the host to port 3000 in the container:

```yaml
ports:
  - "80:3000"  # Host port 80 -> Container port 3000
```

### Linux Port 80 Permissions

On Linux, binding to port 80 requires root privileges. You have several options:

#### Option 1: Run with sudo (simplest)
```bash
sudo docker-compose -f docker-compose.prod.yml up -d
```

#### Option 2: Use a higher port
```bash
# In .env, set:
SERVER_PORT=8080

# Then deploy normally
docker-compose -f docker-compose.prod.yml up -d
```

#### Option 3: Use a reverse proxy (recommended)
See the [Reverse Proxy Setup](#reverse-proxy-setup) section below.

## Required Environment Variables

Create your `.env` file with these essential variables:

```env
# Environment
ENVIRONMENT=production

# Server (port 80 for production)
SERVER_PORT=80

# Database - CHANGE ALL OF THESE!
DB_USERNAME=your_secure_username
DB_PASSWORD=your_secure_password_here
DB_NAMESPACE=slatehub
DB_NAME=production

# S3/MinIO - CHANGE THESE!
S3_ACCESS_KEY_ID=your_access_key
S3_SECRET_ACCESS_KEY=your_secret_key
S3_BUCKET=slatehub

# Security - GENERATE NEW SECRETS!
# Generate with: openssl rand -hex 32
JWT_SECRET=generate_a_secure_random_string_here
SESSION_SECRET=generate_another_secure_random_string_here

# Logging
RUST_LOG=info
LOG_FORMAT=json
```

## Step-by-Step Deployment

### 1. Server Preparation

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Install Docker Compose
sudo apt install docker-compose -y

# Add your user to docker group (optional)
sudo usermod -aG docker $USER
newgrp docker
```

### 2. Configure Firewall

```bash
# Allow SSH (don't lock yourself out!)
sudo ufw allow 22/tcp

# Allow HTTP and HTTPS
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Enable firewall
sudo ufw enable
```

### 3. Build and Deploy

```bash
# Build the production image
make prod-build

# Start services
make prod

# Check status
docker ps

# View logs
docker-compose -f docker-compose.prod.yml logs -f
```

### 4. Verify Deployment

```bash
# Check health endpoint
curl http://localhost/api/health

# Should return:
# {"status":"healthy","database":"connected","version":"0.1.0",...}
```

## Reverse Proxy Setup

### Nginx Configuration

For HTTPS and better security, use Nginx as a reverse proxy:

```nginx
# /etc/nginx/sites-available/slatehub
server {
    listen 80;
    server_name yourdomain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.com/privkey.pem;

    client_max_body_size 100M;

    location / {
        proxy_pass http://localhost:3000;  # Or port 80 if configured
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

### Traefik Configuration

For automatic SSL with Let's Encrypt:

```yaml
# docker-compose.override.yml
services:
  traefik:
    image: traefik:v2.10
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ./traefik/acme.json:/acme.json
    command:
      - --providers.docker=true
      - --entrypoints.web.address=:80
      - --entrypoints.websecure.address=:443
      - --certificatesresolvers.le.acme.email=your@email.com
      - --certificatesresolvers.le.acme.storage=/acme.json
      - --certificatesresolvers.le.acme.httpchallenge.entrypoint=web

  slatehub:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.slatehub.rule=Host(`yourdomain.com`)"
      - "traefik.http.routers.slatehub.entrypoints=websecure"
      - "traefik.http.routers.slatehub.tls.certresolver=le"
```

## Troubleshooting

### Port 80 Already in Use

```bash
# Check what's using port 80
sudo lsof -i :80

# Stop the conflicting service
sudo systemctl stop apache2  # or nginx, etc.
```

### Permission Denied Errors

```bash
# Fix Docker socket permissions
sudo chmod 666 /var/run/docker.sock

# Or add user to docker group
sudo usermod -aG docker $USER
newgrp docker
```

### Container Won't Start

```bash
# Check logs
docker-compose -f docker-compose.prod.yml logs slatehub-server

# Common issues:
# - Missing environment variables: Check .env file
# - Database connection failed: Ensure DB credentials match
# - Port conflicts: Change SERVER_PORT in .env
```

### Health Check Failures

```bash
# Test each service individually
curl http://localhost/api/health  # Application
curl http://localhost:8000/health  # SurrealDB
curl http://localhost:9000/minio/health/live  # MinIO

# Restart unhealthy services
docker-compose -f docker-compose.prod.yml restart slatehub-server
```

## Security Checklist

### Essential Steps

- [ ] **Change all default passwords** in `.env`
- [ ] **Generate secure secrets** for JWT and sessions
- [ ] **Enable HTTPS** with SSL certificates
- [ ] **Configure firewall** to allow only necessary ports
- [ ] **Set up automated backups**
- [ ] **Enable monitoring** and alerting
- [ ] **Review logs** regularly

### Generate Secure Secrets

```bash
# Generate JWT secret
openssl rand -hex 32

# Generate session secret
openssl rand -hex 32

# Generate database password
openssl rand -base64 32
```

### Automated Backups

```bash
# Create backup script
cat > backup.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/backups/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR

# Backup database
docker exec slatehub-surrealdb \
  surreal export --conn http://localhost:8000 \
  --user $DB_USERNAME --pass $DB_PASSWORD \
  --ns slatehub --db production \
  > $BACKUP_DIR/database.sql

# Backup files
tar -czf $BACKUP_DIR/files.tar.gz /var/lib/docker/volumes/slatehub_minio-data

# Keep only last 30 days
find /backups -type d -mtime +30 -exec rm -rf {} +
EOF

chmod +x backup.sh

# Add to crontab
crontab -e
# Add: 0 3 * * * /path/to/backup.sh
```

## Monitoring

### Basic Health Monitoring

```bash
# Create monitoring script
cat > monitor.sh << 'EOF'
#!/bin/bash
if ! curl -f http://localhost/api/health > /dev/null 2>&1; then
  echo "SlateHub is down!" | mail -s "Alert: SlateHub Down" admin@yourdomain.com
  docker-compose -f docker-compose.prod.yml restart slatehub-server
fi
EOF

chmod +x monitor.sh

# Add to crontab (every 5 minutes)
*/5 * * * * /path/to/monitor.sh
```

### Using Prometheus/Grafana

Add to your docker-compose:

```yaml
prometheus:
  image: prom/prometheus
  volumes:
    - ./prometheus.yml:/etc/prometheus/prometheus.yml
  ports:
    - "9090:9090"

grafana:
  image: grafana/grafana
  ports:
    - "3001:3000"
  environment:
    - GF_SECURITY_ADMIN_PASSWORD=admin
```

## Performance Tuning

### Docker Resource Limits

In `docker-compose.prod.yml`:

```yaml
services:
  slatehub:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 1G
        reservations:
          cpus: '1'
          memory: 512M
```

### Database Optimization

```env
# In .env
DB_MAX_CONNECTIONS=100
DB_CONNECTION_TIMEOUT_SECS=5
```

### Nginx Caching

Add to Nginx config:

```nginx
location ~* \.(jpg|jpeg|png|gif|ico|css|js)$ {
    expires 1y;
    add_header Cache-Control "public, immutable";
}
```

## Maintenance

### Update Application

```bash
# Pull latest changes
git pull

# Rebuild and restart
docker-compose -f docker-compose.prod.yml build
docker-compose -f docker-compose.prod.yml up -d
```

### View Logs

```bash
# All services
docker-compose -f docker-compose.prod.yml logs -f

# Specific service
docker-compose -f docker-compose.prod.yml logs -f slatehub-server

# Last 100 lines
docker-compose -f docker-compose.prod.yml logs --tail=100
```

### Restart Services

```bash
# All services
docker-compose -f docker-compose.prod.yml restart

# Specific service
docker-compose -f docker-compose.prod.yml restart slatehub-server
```

## Common Issues and Solutions

### Issue: GLIBC Version Mismatch

**Solution**: The Dockerfile now uses matching Debian versions for build and runtime stages.

### Issue: MinIO Permission Denied

**Solution**: The production compose file now sets `user: root` for MinIO and SurrealDB containers.

### Issue: Can't Access on Port 80

**Solutions**:
1. Check firewall: `sudo ufw status`
2. Check if port is in use: `sudo lsof -i :80`
3. Verify Docker mapping: `docker ps`
4. Try with sudo: `sudo docker-compose up`

### Issue: Database Connection Failed

**Solutions**:
1. Check credentials match in `.env`
2. Ensure SurrealDB is running: `docker ps`
3. Check SurrealDB logs: `docker logs slatehub-surrealdb`
4. Verify network connectivity between containers

## Support

For issues:
1. Check logs: `docker-compose logs`
2. Verify environment: `docker-compose config`
3. Test health: `curl http://localhost/api/health`
4. Review this guide
5. Open an issue on GitHub