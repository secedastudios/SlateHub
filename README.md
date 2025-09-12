# SlateHub
SlateHub is a free, open-source SaaS platform for the TV, film, and content industries. It's an ad-free collaborative hub that combines the networking of LinkedIn with the project management of GitHub. Semantic search and AI-assisted profiles connect filmmakers, creatives, brands, crew, and agencies.

## Configuration

SlateHub uses environment variables for configuration. Copy the `.env.example` file to `.env` and update the values according to your setup:

```bash
cp .env.example .env
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DB_HOST` | Database host address | `localhost` |
| `DB_PORT` | Database port number | `8000` |
| `DB_USERNAME` | Database username | `root` |
| `DB_PASSWORD` | Database password | `root` |
| `DB_NAMESPACE` | Database namespace | `slatehub` |
| `DB_NAME` | Database name | `main` |
| `SERVER_HOST` | Server bind address | `127.0.0.1` |
| `SERVER_PORT` | Server port number | `3000` |
| `DATABASE_URL` | (Optional) Full database connection URL | Constructed from host and port |
| `RUST_LOG` | Log level configuration | `info,slatehub=debug,tower_http=debug` |
| `LOG_FORMAT` | Log output format (`json`, `pretty`, `compact`) | `pretty` |

### Example .env file

```
# Database Connection Configuration
DB_HOST=localhost
DB_PORT=8000

# Database Authentication
DB_USERNAME=root
DB_PASSWORD=root

# Database Namespace and Name
DB_NAMESPACE=slatehub
DB_NAME=main

# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=3000

# Logging Configuration
RUST_LOG=info,slatehub=debug,tower_http=debug
LOG_FORMAT=pretty
```

## Getting Started

1. Clone the repository
2. Copy `.env.example` to `.env` and configure your environment variables
3. Start the database (e.g., using docker-compose)
4. Run the server: `cd server && cargo run`

## Logging

SlateHub uses the `tracing` ecosystem for structured logging, which is the modern standard for Rust applications.

### Configuration Loading Order

The application loads configuration in the following order:

1. **`.env` file is loaded first** - This happens at the very start of the application
2. **Logging is initialized** - Uses `RUST_LOG` and `LOG_FORMAT` from the environment (including `.env`)
3. **Application configuration is loaded** - Database and server settings are read

This ensures that logging configuration from your `.env` file is properly applied. Environment variables always take precedence over `.env` file values.

### Log Levels

The `RUST_LOG` environment variable controls the log level. You can set different levels for different modules:

- `trace` - Very verbose, includes all details
- `debug` - Debugging information
- `info` - General information (default)
- `warn` - Warning messages
- `error` - Error messages only

Example configurations:
- `RUST_LOG=info` - Info level for all modules
- `RUST_LOG=warn,slatehub=debug` - Warn level globally, debug for slatehub
- `RUST_LOG=info,slatehub=debug,tower_http=debug` - Mixed levels (default)

### Log Formats

The `LOG_FORMAT` environment variable controls the output format:

- `pretty` - Human-readable format with colors (default, best for development)
- `json` - JSON structured logs (best for production/log aggregation)
- `compact` - Compact single-line format

### Environment Variable Precedence

The order of precedence for configuration values is:

1. **Command-line environment variables** (highest priority)
   ```bash
   RUST_LOG=trace cargo run
   ```
2. **`.env` file values**
3. **Default values** (lowest priority)

### Viewing Logs

When running the server, you'll see logs like:

```
2025-01-01T12:00:00.123456Z  INFO slatehub: Starting SlateHub server...
2025-01-01T12:00:00.234567Z  INFO slatehub: Database connection established
2025-01-01T12:00:00.345678Z  INFO slatehub: Server listening on: 127.0.0.1:3000
```

For production deployments, use `LOG_FORMAT=json` to get structured logs that can be easily parsed by log aggregation systems.

## Recent Updates

### Profile Image Storage (January 2025)

The profile image upload system has been simplified:

- **Direct URL Storage**: Profile avatars now store the image URL directly in the `person.profile.avatar` field instead of using a separate media table with relationships
- **Improved Performance**: Single database query retrieves the complete profile including the avatar URL
- **Simplified Architecture**: No more complex relationship queries or media record management for profile images
- **MinIO/S3 Integration**: Images are uploaded directly to object storage with automatic thumbnail generation

For detailed information about the profile image upload system, see [Profile Image Upload Documentation](docs/PROFILE_IMAGE_UPLOAD.md).

#### Testing Profile Upload

A test script is provided to verify the profile upload functionality:

```bash
./test/test_profile_upload.sh
```

This will test image upload, storage, and retrieval to ensure everything is working correctly.
