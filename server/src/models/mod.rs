//! Persistence layer: one module per SurrealDB table (or tight cluster).
//!
//! Each model owns every SurrealQL statement touching its table(s) and
//! exposes typed async fns to routes/services. Conventions: bind real
//! `RecordId` values (never "table:key" strings — SurrealDB 3.1 rejects
//! them on record fields), cast ids to strings in SELECTs that
//! deserialize into plain rows, and keep cross-table cascades in
//! `person::Person::delete_with_cascade`.

pub mod activity;
pub mod analytics;
pub mod consent_grant;
pub mod equipment;
pub mod involvement;
pub mod job;
pub mod likes;
pub mod location;
pub mod media;
pub mod membership;
pub mod messaging;
pub mod notification;
pub mod oauth_client;
pub mod organization;
pub mod pending_invitation;
pub mod person;
pub mod production;
pub mod script;
pub mod system;
