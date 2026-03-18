# SlateHub — The Free Networking Platform for Film, TV & Content Creators

[![Website](https://img.shields.io/badge/website-slatehub.com-eb5437)](https://slatehub.com)
[![License](https://img.shields.io/badge/license-open--source-green)](#)

**[SlateHub](https://slatehub.com)** is a free, open-source platform where filmmakers, actors, crew, creators, and brands connect to turn ideas into stories. No ads, no subscriptions — just networking, smart search, and verified connections.

## Why SlateHub?

- **Free Forever** — No subscriptions, no ads. Free for all creatives, for life.
- **Smart Search** — Semantic search powered by AI connects you with the right talent, roles, and projects instantly.
- **Verified Accounts** — One-time identity verification builds trust and reduces spam.
- **AI-Powered Profiles** — Auto-build your profile from your existing links and portfolios.
- **Production Management** — Create productions, invite crew, assign roles, and manage your team.
- **Direct Messaging** — Message other creatives directly on the platform.
- **Job Board** — Post and discover job opportunities in the creative industry.
- **Global & Inclusive** — From emerging talent to industry pros, SlateHub empowers everyone.

## Who It's For

- **Actors & Talent** — Showcase reels, credits, and connections to get cast without barriers.
- **Crew Members** — Highlight your skills and past projects to land the perfect gig.
- **Filmmakers & Directors** — Find talent and collaborators to bring your vision to life.
- **Creators & Influencers** — Professionalize your brand and connect with opportunities.
- **Producers & Brands** — Scout talent, manage productions, and post jobs with ease.
- **Organizations** — Studios, agencies, and production companies can build a presence and manage teams.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Backend | [Rust](https://www.rust-lang.org/) + [Axum](https://github.com/tokio-rs/axum) |
| Database | [SurrealDB](https://surrealdb.com) (document, graph, vector) |
| Templates | [Askama](https://github.com/djc/askama) (server-side HTML) |
| Storage | [RustFS](https://rustfs.com) (S3-compatible object storage) |
| Search | Vector embeddings (BGE-Large-EN-v1.5, 1024 dimensions) with HNSW indexes |
| Email | [Mailjet](https://www.mailjet.com/) |

## Getting Started

1. Clone the repository
2. Copy `.env.example` to `.env` and configure your environment variables
3. Start all services with Docker: `make services-start`
4. Initialize the database: `make db-init`
5. Run the server: `make dev` (local) or `make start` (Docker)

### Services

SlateHub depends on two backing services, both managed via `docker-compose`:

| Service | Purpose | Default ports |
|---------|---------|---------------|
| [SurrealDB](https://surrealdb.com) | Primary database | `8000` |
| [RustFS](https://rustfs.com) | S3-compatible object storage | API `9000`, Console `9001` |

RustFS is a high-performance, Apache 2.0-licensed S3-compatible object store written in Rust.
Profile images and organisation logos are stored there and served publicly without presigned URLs.
The server talks to RustFS (and any other S3-compatible backend) through the standard AWS S3 SDK —
swap the `S3_ENDPOINT`, `S3_ACCESS_KEY`, and `S3_SECRET_KEY` variables to point at AWS S3,
Cloudflare R2, or any other compatible service instead.

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
| `APP_URL` | Public URL of the application | `http://localhost:3000` |
| `RUST_LOG` | Log level configuration | `info,slatehub=debug,tower_http=debug` |
| `LOG_FORMAT` | Log output format (`json`, `pretty`, `compact`) | `pretty` |
| `S3_ENDPOINT` | S3-compatible storage endpoint URL | `http://localhost:9000` |
| `S3_ACCESS_KEY` | S3 access key | `admin` |
| `S3_SECRET_KEY` | S3 secret key | `password` |
| `S3_BUCKET` | S3 bucket name | `slatehub` |
| `S3_REGION` | S3 region | `us-east-1` |
| `MAILJET_API_KEY` | Mailjet API key for sending emails | Required for email features |
| `MAILJET_API_SECRET` | Mailjet API secret | Required for email features |
| `MAILJET_FROM_EMAIL` | Default sender email address | `noreply@slatehub.com` |
| `MAILJET_FROM_NAME` | Default sender name | `SlateHub` |

## Semantic Search

SlateHub uses vector embeddings (BGE-Large-EN-v1.5, 1024 dimensions) for semantic search across people, organizations, locations, and productions. Embeddings are automatically generated when records are created or updated.

To rebuild all embeddings from scratch (e.g. after a schema change or model upgrade):

```bash
make rebuild-embeddings
```

## Testing

```bash
# Run all tests with automatic setup/teardown
make test

# Run only unit tests
make test-unit

# Run only integration tests
make test-integration

# Watch and auto-run tests on file changes
make test-watch

# Generate coverage report
make test-coverage
```

The test environment runs on separate ports (SurrealDB: 8100, RustFS: 9100/9101) to avoid interfering with development data. For detailed testing documentation, see [Testing Guide](docs/TESTING.md).

## Logging

SlateHub uses the `tracing` ecosystem for structured logging.

- `RUST_LOG` controls log levels (e.g. `info`, `warn,slatehub=debug`, `trace`)
- `LOG_FORMAT` controls output format: `pretty` (dev), `json` (production), `compact`
- Environment variables override `.env` file values

## Contributing

SlateHub is open source and welcomes contributions. Whether it's bug fixes, new features, or documentation improvements — all contributions help build a better platform for the creative community.

## Links

- **Website**: [slatehub.com](https://slatehub.com)
- **Search for Talent**: [slatehub.com/search](https://slatehub.com/search)
- **Browse People**: [slatehub.com/people](https://slatehub.com/people)
- **Browse Productions**: [slatehub.com/productions](https://slatehub.com/productions)
- **Browse Organizations**: [slatehub.com/orgs](https://slatehub.com/orgs)
- **Browse Locations**: [slatehub.com/locations](https://slatehub.com/locations)
- **Browse Jobs**: [slatehub.com/jobs](https://slatehub.com/jobs)
