//! Service layer: cross-cutting integrations and domain services that sit
//! between the route handlers and the models.
//!
//! Services own the things that aren't a single table's CRUD: third-party
//! APIs (Stripe, Mailjet, TMDB, Listmonk, S3), in-process engines (fastembed,
//! the aristotle screenplay pipeline), background workers (SSF delivery,
//! notification fan-out, stale-payment refunds), and multi-model workflows
//! (invitations, OIDC token issuance). Route handlers call services; services
//! call models and/or the global `crate::db::DB` connection directly.
//!
//! Most services are initialized once from `main.rs` at boot (see each
//! module's docs for its init function and required env vars); the rest are
//! constructed per call from env or are plain stateless functions.
//!
//! # Module index
//!
//! | Module | Purpose |
//! |---|---|
//! | [`activity`] | Fire-and-forget `activity_event` rows for page views (spawned, never blocks) |
//! | [`aristotle_runner`] | Concurrency-capped wrapper running the in-crate aristotle script-breakdown pipeline |
//! | [`email`] | Transactional email (verification, password reset, invitations, feedback) via the Mailjet API |
//! | [`embedding`] | In-process fastembed (BGE-Large-EN-v1.5) vectors + embedding-text builders for semantic search |
//! | [`feature_flag`] | Code-registered, DB-configured feature flags with four visibility states |
//! | [`geodata`] | Static city → region/country lookup used to enrich embedding text |
//! | [`invitation`] | Org/production invites for existing users (membership + notification) and unknown emails (pending row + email) |
//! | [`listmonk`] | Best-effort newsletter subscription fan-out to a self-hosted Listmonk instance |
//! | [`notification_stream`] | SurrealDB `LIVE SELECT` on `notification` bridged to a tokio broadcast channel for SSE |
//! | [`oidc_events`] | Outbound SSF/CAEP/RISC Security Event Tokens with a retrying background delivery worker |
//! | [`oidc_keys`] | ed25519 OIDC signing keypair: generation, JWKS publication, id_token signing, rotation |
//! | [`oidc_tokens`] | OIDC authorization codes + access/refresh tokens: issuance, hashing, lookup, revocation |
//! | [`s3`] | S3-compatible object storage (RustFS/MinIO/AWS) for uploads, downloads, presigned URLs |
//! | [`search`] | Canonical layered search queries (people/orgs/locations/productions/jobs) shared by web + MCP |
//! | [`search_log`] | Fire-and-forget `search_log` rows recording query + result counts |
//! | [`search_utils`] | Query normalization and natural-language filter parsing for people search |
//! | [`stripe`] | Stripe Checkout + Identity + refunds over raw REST, with manual webhook signature verification |
//! | [`tmdb`] | TMDB person search + combined credits for profile credit import |
//! | [`verification`] | Six-digit email-verification / password-reset codes in `verification_codes` |

pub mod activity;
pub mod aristotle_runner;
pub mod email;
pub mod embedding;
pub mod feature_flag;
pub mod geodata;
pub mod invitation;
pub mod listmonk;
pub mod notification_stream;
pub mod oidc_events;
pub mod oidc_keys;
pub mod oidc_tokens;
pub mod s3;
pub mod search;
pub mod search_log;
pub mod search_utils;
pub mod stripe;
pub mod tmdb;
pub mod verification;
