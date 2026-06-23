//! SlateHub — a self-hosted networking and production-management platform
//! for the film/TV industry, built on Axum + SurrealDB + Askama + Datastar.
//!
//! # Layering
//!
//! - [`routes`] — HTTP surface: one module per page area, each exposing a
//!   `router()`. Handlers stay thin: extract, call a model/service, render.
//! - [`models`] — persistence layer; each module owns one SurrealDB table
//!   (or a tight cluster) and all SurrealQL touching it.
//! - [`services`] — cross-cutting integrations and domain services (S3,
//!   Stripe, email, embeddings, feature flags, the aristotle runner …).
//! - [`templates`] — Askama template structs + shared filters; the bridge
//!   between handler data and `templates/*.html`.
//! - [`middleware`] — auth extraction, request ids, error-shape negotiation,
//!   activity tracking.
//! - [`aristotle`] — the inlined screenplay parsing/breakdown engine
//!   (self-contained; extractable back into its own crate).
//!
//! Shared plumbing: [`error`] (the crate-wide `Error`/`Result`), [`db`] (the
//! global SurrealDB handle), [`auth`] (JWT + password hashing), [`config`],
//! [`datastar`]/[`html`]/[`text`] (fragment + formatting helpers).

pub mod aristotle;
pub mod auth;
pub mod config;
pub mod datastar;
pub mod db;
pub mod error;
pub mod html;
pub mod logging;
pub mod markdown;
pub mod mcp;
pub mod middleware;
pub mod models;
pub mod record_id_ext;
pub mod response;
pub mod routes;
pub mod serde_utils;
pub mod services;
pub mod social_platforms;
pub mod stats;
pub mod templates;
pub mod text;
pub mod verification_limits;
pub mod version;
pub mod video_platforms;
