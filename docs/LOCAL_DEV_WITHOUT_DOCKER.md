# Local Development Without Docker

The `Makefile` assumes Docker for both backing services (`make services` →
`docker-compose up surrealdb rustfs`), and `make db-init` / `make db-seed`
reach into the container with `docker exec`. On a machine without Docker,
none of those targets work.

This document records the Docker-free path: what the server actually needs to
boot, and the native equivalent of each `make` target.

## What the server actually requires

From `server/src/main.rs`:

| Dependency | Required? | Behaviour when missing |
|---|---|---|
| **SurrealDB** | **Yes** | Retries 10× at 2s intervals, then exits (`main.rs:58-83`) |
| RustFS / S3 | No | Logs an error, continues. Only image uploads break (`main.rs:143-150`) |
| Embedding model | No | Logs an error, continues. Only semantic search breaks (`main.rs:153-165`) |
| Postmark / Mailjet | No | Email sending fails; the rest of the app is unaffected |
| Stripe / Listmonk / TMDB | No | Feature-flagged off when unset |

So a working local site needs **SurrealDB and nothing else**.

## Setup

### 1. Install SurrealDB natively

The version must match the `surrealdb` client crate in `server/Cargo.toml`
(currently **3.1.4**).

Homebrew (`brew install surrealdb/tap/surreal`) is the tidiest route, but it
fails on machines whose Command Line Tools are older than the installed macOS
expects:

```
Error: Your Command Line Tools are too outdated.
```

Homebrew requires up-to-date CLT even for a formula that only unpacks a
prebuilt binary. If you hit that and don't want to update Xcode CLT, take the
official prebuilt binary instead — no compiler involved:

```bash
curl -sSL -o surreal.tgz https://github.com/surrealdb/surrealdb/releases/download/v3.1.4/surreal-v3.1.4.darwin-arm64.tgz && tar -xzf surreal.tgz && mkdir -p ~/.local/bin && mv surreal ~/.local/bin/ && chmod +x ~/.local/bin/surreal
```

Use the `darwin-amd64` asset on Intel Macs. Confirm with `surreal version`.

### 2. Start SurrealDB

Replaces `make services` (SurrealDB half — see "RustFS" below for the rest):

```bash
surreal start --username root --password root --bind 127.0.0.1:8000 --log info "rocksdb:$HOME/.local/share/slatehub/data/database.db"
```

**Keep the data path free of spaces.** RocksDB storage paths containing spaces
have caused failures on this setup before, and the repo's default location
(`db/data`) inherits any spaces from the checkout's parent directories. Storing
data under `~/.local/share/slatehub/` sidesteps it. That also means the data
lives outside the repo, so `make clean` will not remove it — delete the
directory by hand if you want a truly fresh database.

Verify: `curl http://127.0.0.1:8000/health` → `200`.

### 3. Configure `.env`

Copy `.env.example` to `.env` and change:

| Variable | Value | Why |
|---|---|---|
| `DB_HOST` | `localhost` | The example uses `surrealdb`, the docker-compose service name, which does not resolve outside the compose network |
| `SERVER_PORT` | e.g. `3001` | Only if something already holds port 3000 |
| `APP_URL` | must match `SERVER_PORT` | Used for links in emails and redirects |
| `ENVIRONMENT` | `development` | The example ships `production` |
| `META_PIXEL_ID` | *(blank)* | Keeps local test traffic off the real Meta pixel |
| `POSTMARK_SERVER_TOKEN` | *(blank)* | Prevents local runs from sending real mail through the production server |
| `JWT_SECRET`, `SESSION_SECRET` | throwaway values | Never reuse production secrets locally |

`.env` is gitignored (`.gitignore` line 6), so none of this reaches a PR.

Note that `make dev` runs `cd server && cargo run`, while `.env` sits at the
repo root — that works because the `dotenv` crate walks up parent directories
to find it.

### 4. Initialise the schema

Replaces `make db-init`, which pipes `db/schema.surql` through
`docker exec … /surreal import`:

```bash
surreal import --endpoint http://localhost:8000 --username root --password root --namespace slatehub --database main db/schema.surql
```

### 5. Seed data (optional)

Replaces `make db-seed`. Use the HTTP `/sql` endpoint rather than
`surreal sql` — the interactive REPL parses input line by line and chokes on
the multi-line statements in the seed files:

```bash
curl -sf -X POST "http://localhost:8000/sql" -H "Accept: application/json" -H "surreal-ns: slatehub" -H "surreal-db: main" -u "root:root" --data-binary @db/seed-verified-people.surql
```

The users/org/production block that `make db-seed` inlines (Makefile:192-267)
can be saved to a `.surql` file and posted the same way. Seeded logins are
`kevin` / `pass123` and `chris` / `pass123` (admin).

### 6. Run the server

`make dev` still works, since it only shells out to `cargo`:

```bash
make dev
```

First boot downloads the BGE-Large-EN-v1.5 weights (~1.3 GB) into
`server/.fastembed_cache/` before the listener binds, so it can sit on
"Initializing embedding service" for several minutes with no output. This is a
one-time cost; the directory is gitignored. The server is ready when the log
reads `SlateHub server is ready to accept connections`.

## RustFS / S3

Not covered here — RustFS is only distributed as a container image, so it needs
Docker. Without it the server logs at startup:

```
ERROR slatehub: Failed to initialize S3 service: … Connection refused (os error 61)
ERROR slatehub: Warning: Profile image uploads will not work without S3 service
```

Everything except avatar and logo uploads behaves normally. If you need uploads
locally, either install Docker or point `S3_ENDPOINT`, `S3_ACCESS_KEY` and
`S3_SECRET_KEY` at any other S3-compatible service — the server uses the
standard AWS S3 SDK and does not depend on RustFS specifically.

## Targets that still need Docker

`make start`, `make stop`, `make server`, `make build`, `make services`,
`make db-init`, `make db-seed`, `make db-drop`, `make logs`, `make shell`,
`make clean`, `make purge`, and the whole `make test` chain
(`docker-compose.test.yml`) all invoke `docker` or `docker-compose` directly.
Only `make dev`, `make unstick`, `make rebuild-embeddings` and `make db-migrate`
(which uses `curl`, not `docker exec`) work without it.
