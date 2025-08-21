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
```

## Getting Started

1. Clone the repository
2. Copy `.env.example` to `.env` and configure your environment variables
3. Start the database (e.g., using docker-compose)
4. Run the server: `cd server && cargo run`
