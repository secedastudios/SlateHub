//! Stripe Payments + Stripe Identity integration.
//!
//! We use raw HTTP against the Stripe REST API rather than `async-stripe` —
//! the surface area we need is small (~5 endpoints) and `async-stripe`'s
//! current major version requires reqwest 0.12 (we're on 0.11). Webhook
//! signatures are verified manually with `hmac` + a constant-time compare.
//!
//! Pricing is hardcoded as a multi-currency table. Customer's preferred
//! currency is sniffed from `Accept-Language`, falling back to USD. Stripe
//! Tax handles VAT calculation on the Stripe side (must be enabled in the
//! dashboard) — we pass `automatic_tax: { enabled: true }` and the price as
//! VAT-inclusive (the price the customer sees).

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tracing::{debug, info, warn};

use crate::error::{Error, Result};

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// Pricing
// ---------------------------------------------------------------------------

/// Multi-currency price table. Amounts are in **minor units** (cents/pence/
/// öre/etc.) and are **VAT-inclusive** for jurisdictions where the law
/// requires inclusive display. Stripe Tax does the actual breakdown.
///
/// Round numbers per currency — we don't FX-convert daily. Add/adjust as
/// needed; anyone whose locale doesn't match falls through to USD.
pub const PRICE_TABLE: &[Price] = &[
    Price {
        currency: "eur",
        amount_minor: 1000,
        label: "€10.00",
    },
    Price {
        currency: "usd",
        amount_minor: 1000,
        label: "$10.00",
    },
    Price {
        currency: "gbp",
        amount_minor: 900,
        label: "£9.00",
    },
    Price {
        currency: "cad",
        amount_minor: 1400,
        label: "CA$14.00",
    },
    Price {
        currency: "aud",
        amount_minor: 1500,
        label: "AU$15.00",
    },
    Price {
        currency: "chf",
        amount_minor: 1000,
        label: "10 CHF",
    },
    Price {
        currency: "jpy",
        amount_minor: 1500,
        label: "¥1,500",
    },
];

#[derive(Debug, Clone, Copy)]
pub struct Price {
    pub currency: &'static str,
    pub amount_minor: i64,
    pub label: &'static str,
}

/// Pick a Price for the user based on an Accept-Language header. Conservative
/// mapping — when in doubt, USD. Real i18n is a bigger fish; this gets us
/// 90% there for the launch.
pub fn pick_price(accept_language: Option<&str>) -> &'static Price {
    let default = PRICE_TABLE
        .iter()
        .find(|p| p.currency == "usd")
        .expect("USD price always present");

    let Some(raw) = accept_language else {
        return default;
    };
    let lang = raw.to_lowercase();

    // Country-tagged forms first (more specific than language alone).
    if lang.contains("en-gb") || lang.contains("en_gb") {
        return find("gbp").unwrap_or(default);
    }
    if lang.contains("en-ca") || lang.contains("fr-ca") {
        return find("cad").unwrap_or(default);
    }
    if lang.contains("en-au") {
        return find("aud").unwrap_or(default);
    }
    if lang.contains("de-ch") || lang.contains("fr-ch") || lang.contains("it-ch") {
        return find("chf").unwrap_or(default);
    }
    if lang.starts_with("ja") {
        return find("jpy").unwrap_or(default);
    }
    // Bare language → EUR for the main eurozone languages.
    for euro in [
        "de", "fr", "it", "es", "nl", "pt", "el", "fi", "sv", "da", "pl", "cs",
    ] {
        if lang.starts_with(euro) {
            return find("eur").unwrap_or(default);
        }
    }
    default
}

fn find(currency: &str) -> Option<&'static Price> {
    PRICE_TABLE.iter().find(|p| p.currency == currency)
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StripeService {
    secret_key: String,
    webhook_secret: String,
    http: reqwest::Client,
}

impl StripeService {
    /// Build from env. `STRIPE_SECRET_KEY` and `STRIPE_WEBHOOK_SECRET` are
    /// required; if missing, returns `None` (callers no-op rather than panic).
    pub fn from_env() -> Option<Self> {
        let secret_key = std::env::var("STRIPE_SECRET_KEY").ok()?;
        let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").ok()?;
        if secret_key.is_empty() || webhook_secret.is_empty() {
            return None;
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .ok()?;
        Some(Self {
            secret_key,
            webhook_secret,
            http,
        })
    }

    /// Log integration status at boot.
    pub fn log_status() {
        match Self::from_env() {
            Some(_) => info!("stripe: configured"),
            None => info!("stripe: env incomplete — paid verification disabled"),
        }
    }

    // ---- Checkout ------------------------------------------------------

    /// Create a one-time Checkout Session for the identity-verification fee.
    /// Returns the session id and the URL to redirect the customer to.
    pub async fn create_checkout_session(
        &self,
        person_id: &str,
        customer_email: &str,
        price: &Price,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSessionCreated> {
        // Stripe's API uses application/x-www-form-urlencoded with nested
        // bracket syntax. We build the form by hand to keep the dependency
        // surface tight.
        let amount = price.amount_minor.to_string();
        let form: Vec<(&str, &str)> = vec![
            ("mode", "payment"),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
            ("customer_email", customer_email),
            // Force Stripe to email a receipt independent of the dashboard
            // "Email customers about successful payments" setting. In test
            // mode the dashboard default is off; this makes test-mode
            // receipts visible too.
            ("payment_intent_data[receipt_email]", customer_email),
            ("automatic_tax[enabled]", "true"),
            ("client_reference_id", person_id),
            ("metadata[person_id]", person_id),
            ("metadata[purpose]", "identity_verification"),
            ("line_items[0][quantity]", "1"),
            ("line_items[0][price_data][currency]", price.currency),
            ("line_items[0][price_data][unit_amount]", &amount),
            ("line_items[0][price_data][tax_behavior]", "inclusive"),
            (
                "line_items[0][price_data][product_data][name]",
                "SlateHub Identity Verification",
            ),
            (
                "line_items[0][price_data][product_data][description]",
                "One-time identity verification fee. Refunded if verification fails or is not completed within 24 hours.",
            ),
            (
                "line_items[0][price_data][product_data][tax_code]",
                "txcd_10000000",
            ),
        ];

        debug!(person_id, currency = %price.currency, "creating Stripe Checkout Session");

        let resp: CheckoutSessionCreated = self.post_form("/v1/checkout/sessions", &form).await?;

        info!(
            person_id,
            session = %resp.id,
            url = %resp.url,
            "Stripe Checkout Session created"
        );
        Ok(resp)
    }

    /// Retrieve a Checkout Session — used in the return handler to confirm
    /// payment status before creating an Identity session.
    pub async fn retrieve_checkout_session(&self, session_id: &str) -> Result<CheckoutSession> {
        self.get(&format!("/v1/checkout/sessions/{}", session_id))
            .await
    }

    // ---- Identity ------------------------------------------------------

    /// Create a Stripe Identity VerificationSession. Returns the session
    /// id and the hosted-flow URL to redirect the customer to.
    pub async fn create_identity_session(
        &self,
        person_id: &str,
        return_url: &str,
    ) -> Result<IdentitySessionCreated> {
        let form: Vec<(&str, &str)> = vec![
            ("type", "document"),
            ("return_url", return_url),
            ("metadata[person_id]", person_id),
            ("options[document][require_matching_selfie]", "true"),
            ("options[document][require_live_capture]", "true"),
            // Stripe's form parser requires consistent indexed-array notation;
            // mixing `[]` (append) with `[N]` (positional) returns 400.
            ("options[document][allowed_types][0]", "driving_license"),
            ("options[document][allowed_types][1]", "passport"),
            ("options[document][allowed_types][2]", "id_card"),
        ];

        debug!(person_id, "creating Stripe Identity VerificationSession");

        let resp: IdentitySessionCreated = self
            .post_form("/v1/identity/verification_sessions", &form)
            .await?;

        info!(
            person_id,
            session = %resp.id,
            "Stripe Identity VerificationSession created"
        );
        Ok(resp)
    }

    /// Retrieve a previously-created Identity session. Returns the live
    /// `url` (when status is still `requires_input` or `processing`) so we
    /// can redirect the user back into Stripe's hosted flow exactly where
    /// they left off — same QR code, same scan-to-phone link, same partial
    /// uploads.
    pub async fn retrieve_identity_session(&self, session_id: &str) -> Result<IdentitySession> {
        self.get(&format!(
            "/v1/identity/verification_sessions/{}",
            session_id
        ))
        .await
    }

    // ---- Refunds -------------------------------------------------------

    /// Issue a full refund for a payment intent.
    pub async fn refund(&self, payment_intent_id: &str, reason: RefundReason) -> Result<Refund> {
        let form: Vec<(&str, &str)> = vec![
            ("payment_intent", payment_intent_id),
            ("reason", reason.as_stripe_str()),
        ];

        debug!(payment_intent_id, "issuing Stripe refund");
        let refund: Refund = self.post_form("/v1/refunds", &form).await?;
        info!(payment_intent_id, refund_id = %refund.id, "Stripe refund issued");
        Ok(refund)
    }

    // ---- Webhook signature --------------------------------------------

    /// Verify a Stripe webhook signature. Stripe sends a header like
    /// `t=<timestamp>,v1=<signature>,v1=<signature2>` (multiple v1's during
    /// secret rotation). We accept the request if any `v1` matches the HMAC.
    ///
    /// Rejects signatures whose timestamp is more than 5 minutes old to
    /// guard against replay.
    pub fn verify_webhook(&self, body: &[u8], signature_header: &str) -> Result<()> {
        let mut timestamp: Option<i64> = None;
        let mut sigs: Vec<&str> = Vec::new();
        for part in signature_header.split(',') {
            let mut kv = part.splitn(2, '=');
            let (k, v) = match (kv.next(), kv.next()) {
                (Some(k), Some(v)) => (k.trim(), v.trim()),
                _ => continue,
            };
            match k {
                "t" => timestamp = v.parse().ok(),
                "v1" => sigs.push(v),
                _ => {}
            }
        }
        let ts = timestamp.ok_or_else(|| Error::BadRequest("missing webhook timestamp".into()))?;
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return Err(Error::BadRequest(
                "webhook timestamp out of tolerance".into(),
            ));
        }
        if sigs.is_empty() {
            return Err(Error::BadRequest("missing webhook signature".into()));
        }

        let signed_payload = format!("{}.{}", ts, std::str::from_utf8(body).unwrap_or(""));
        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .map_err(|e| Error::Internal(format!("hmac key error: {e}")))?;
        mac.update(signed_payload.as_bytes());
        let computed = mac.finalize().into_bytes();

        for sig in sigs {
            let provided = match hex::decode(sig) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if provided.len() == computed.len() && provided.ct_eq(&computed).into() {
                return Ok(());
            }
        }
        Err(Error::BadRequest("webhook signature mismatch".into()))
    }

    // ---- Internals -----------------------------------------------------

    async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("https://api.stripe.com{}", path);
        let resp = self
            .http
            .post(&url)
            .basic_auth(&self.secret_key, Some(""))
            .form(form)
            .send()
            .await
            .map_err(|e| Error::ExternalService(format!("stripe POST {path}: {e}")))?;
        Self::parse_response(resp, path).await
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("https://api.stripe.com{}", path);
        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.secret_key, Some(""))
            .send()
            .await
            .map_err(|e| Error::ExternalService(format!("stripe GET {path}: {e}")))?;
        Self::parse_response(resp, path).await
    }

    async fn parse_response<T: serde::de::DeserializeOwned>(
        resp: reqwest::Response,
        path: &str,
    ) -> Result<T> {
        let status = resp.status();
        if status.is_success() {
            resp.json::<T>()
                .await
                .map_err(|e| Error::ExternalService(format!("stripe parse {path}: {e}")))
        } else {
            let body = resp.text().await.unwrap_or_default();
            warn!(path, status = %status, body = %body, "stripe error response");
            Err(Error::ExternalService(format!(
                "stripe {path} returned {status}: {body}"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Response types (subset of the Stripe object — we deserialize only the
// fields we actually use). Stripe never removes fields, but it may rename
// or add — those won't break us.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionCreated {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub payment_status: String, // "paid" | "unpaid" | "no_payment_required"
    pub status: Option<String>, // "complete" | "open" | "expired"
    pub customer_email: Option<String>,
    pub payment_intent: Option<String>,
    pub amount_total: Option<i64>,
    pub currency: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySessionCreated {
    pub id: String,
    pub url: String,
    pub status: String,
}

/// Retrieved Identity session — `url` is `None` once Stripe is done with
/// the session (verified or canceled). `last_error` carries the reason
/// when status is `requires_input`.
#[derive(Debug, Clone, Deserialize)]
pub struct IdentitySession {
    pub id: String,
    pub status: String,
    pub url: Option<String>,
    pub last_error: Option<IdentityLastError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentityLastError {
    pub code: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund {
    pub id: String,
    pub status: Option<String>,
    pub amount: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Copy)]
pub enum RefundReason {
    RequestedByCustomer,
    Duplicate,
    Fraudulent,
}

impl RefundReason {
    fn as_stripe_str(self) -> &'static str {
        match self {
            Self::RequestedByCustomer => "requested_by_customer",
            Self::Duplicate => "duplicate",
            Self::Fraudulent => "fraudulent",
        }
    }
}

// Webhook event envelope. We only look at `type` + the embedded object.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: WebhookEventData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookEventData {
    pub object: serde_json::Value,
}

/// Refund any verification_payment rows that have been in `paid` state for
/// more than the given number of hours without becoming `verified`. Intended
/// to be called from a daily background task. Skips rows without a
/// `stripe_payment_intent_id` (something is off — admin needs to look).
pub async fn refund_stale_payments(hours: i64) {
    let Some(stripe) = StripeService::from_env() else {
        debug!("refund_stale_payments: stripe not configured, skipping");
        return;
    };

    use surrealdb::types::SurrealValue;
    #[derive(Debug, Deserialize, SurrealValue)]
    struct Row {
        id: surrealdb::types::RecordId,
        stripe_payment_intent_id: Option<String>,
    }

    let sql = format!(
        "SELECT id, stripe_payment_intent_id FROM verification_payment \
         WHERE status = 'paid' AND created_at < time::now() - {}h",
        hours
    );

    let rows: Vec<Row> = match crate::db::DB.query(&sql).await {
        Ok(mut r) => r.take(0).unwrap_or_default(),
        Err(e) => {
            warn!(error = %e, "refund_stale_payments: query failed");
            return;
        }
    };

    if rows.is_empty() {
        return;
    }
    info!(
        count = rows.len(),
        "refund_stale_payments: refunding stale payments"
    );

    for row in rows {
        let Some(pi) = row.stripe_payment_intent_id else {
            warn!(payment = ?row.id, "stale payment has no payment_intent — needs admin attention");
            continue;
        };
        match stripe.refund(&pi, RefundReason::RequestedByCustomer).await {
            Ok(refund) => {
                if let Err(e) = crate::db::DB
                    .query("UPDATE $id SET status = 'refunded', refund_id = $rid, updated_at = time::now()")
                    .bind(("id", row.id.clone()))
                    .bind(("rid", refund.id))
                    .await
                {
                    warn!(error = %e, "failed to mark payment refunded");
                }
            }
            Err(e) => warn!(payment = ?row.id, error = %e, "refund failed"),
        }
    }
}
