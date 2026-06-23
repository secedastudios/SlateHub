//! Identity-verification routes under `/get-verified`: the landing page,
//! free manual-review requests, and the paid Stripe flow (Checkout →
//! Identity session → done page), including resuming a paid-but-unfinished
//! Identity session without charging twice. Paid flow availability is gated
//! by the `identity_verification` feature flag plus Stripe configuration.

use askama::Template;
use axum::{
    Router,
    extract::{Query, Request},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use surrealdb::types::{RecordId, SurrealValue};

use crate::record_id_ext::RecordIdExt;
use tracing::{error, info, warn};

use crate::{
    db::DB,
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    services::feature_flag,
    services::stripe::{StripeService, pick_price},
    templates::{BaseContext, GetVerifiedDoneTemplate, GetVerifiedTemplate, User},
};

/// Routes under `/get-verified` for the verification landing page and the
/// manual-request, checkout, resume, return, and done steps.
pub fn router() -> Router {
    Router::new()
        .route("/get-verified", get(get_verified_page))
        .route("/get-verified/request", post(request_verification))
        .route("/get-verified/checkout", post(start_checkout))
        .route("/get-verified/resume", post(resume_verification))
        .route("/get-verified/return", get(checkout_return))
        .route("/get-verified/done", get(done_page))
}

async fn has_pending_verification(person_id: &RecordId) -> bool {
    if let Ok(mut result) = DB
        .query("SELECT count() AS c FROM verification_request WHERE person = $pid AND status = 'pending' GROUP ALL")
        .bind(("pid", person_id.clone()))
        .await
        && let Ok(Some(row)) = result.take::<Option<serde_json::Value>>(0)
            && let Some(c) = row.get("c").and_then(|v| v.as_i64()) {
                return c > 0;
            }
    false
}

/// Most recent verification_payment status for this person, if any.
async fn latest_payment_status(person_id: &RecordId) -> Option<String> {
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        status: String,
        // Present in SELECT for v3's ORDER BY rule; unused in Rust.
        #[allow(dead_code)]
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let mut response = DB
        .query("SELECT status, created_at FROM verification_payment WHERE person = $pid ORDER BY created_at DESC LIMIT 1")
        .bind(("pid", person_id.clone()))
        .await
        .ok()?;
    let row: Option<Row> = response.take(0).ok().flatten();
    row.map(|r| r.status)
}

async fn is_identity_verified(person_id: &RecordId) -> bool {
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        verification_status: String,
    }
    let mut response = match DB
        .query("SELECT verification_status FROM $pid")
        .bind(("pid", person_id.clone()))
        .await
    {
        Ok(r) => r,
        Err(_) => return false,
    };
    let row: Option<Row> = response.take(0).ok().flatten();
    row.map(|r| r.verification_status == "identity")
        .unwrap_or(false)
}

async fn get_verified_page(request: Request) -> Result<Response, Error> {
    let mut base = BaseContext::new().with_page("get-verified");
    let mut pending = false;
    let mut last_status: Option<String> = None;
    let mut already_verified = false;
    let flag_allowed;

    if let Some(user) = request.get_user() {
        flag_allowed = feature_flag::allows("identity_verification", Some(&user)).await;
        base = base.with_user(User::from_session_user(&user).await);
        if let Ok(rid) = user.record_id() {
            pending = has_pending_verification(&rid).await;
            last_status = latest_payment_status(&rid).await;
            already_verified = is_identity_verified(&rid).await;
        }
    } else {
        flag_allowed = feature_flag::allows("identity_verification", None).await;
    }

    // Price shown on the page is derived from Accept-Language; the *actual*
    // price charged at Checkout uses the same picker, so the displayed label
    // matches what the user will see in Stripe's hosted UI.
    let accept_language = request
        .headers()
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok());
    let price = pick_price(accept_language);
    let paid_flow_enabled = flag_allowed && StripeService::from_env().is_some();

    let template = crate::with_base!(GetVerifiedTemplate, base, {
        has_pending_request: pending,
        already_verified,
        paid_flow_enabled,
        price_label: price.label.to_string(),
        last_payment_status: last_status,
    });

    let html = template.render().map_err(|e| {
        error!("Failed to render get-verified template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

async fn request_verification(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    let rid = user.record_id()?;

    if has_pending_verification(&rid).await {
        return Ok(Redirect::to("/get-verified").into_response());
    }

    if let Err(e) = DB
        .query("CREATE verification_request SET person = $pid, status = 'pending', created_at = time::now()")
        .bind(("pid", rid))
        .await
    {
        error!("Failed to create verification request: {}", e);
    }

    Ok(Redirect::to("/get-verified").into_response())
}

// ---------------------------------------------------------------------------
// Paid flow — Checkout → Identity → Webhook
// ---------------------------------------------------------------------------

async fn start_checkout(
    AuthenticatedUser(user): AuthenticatedUser,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let rid = user.record_id()?;

    if is_identity_verified(&rid).await {
        return Ok(Redirect::to("/get-verified?status=already_verified").into_response());
    }

    if !feature_flag::allows("identity_verification", Some(&user)).await {
        return Err(Error::BadRequest(
            "Paid verification is currently unavailable. Please request manual verification."
                .into(),
        ));
    }

    let Some(stripe) = StripeService::from_env() else {
        return Err(Error::BadRequest(
            "Paid verification is currently unavailable. Please request manual verification."
                .into(),
        ));
    };

    let accept_language = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|h| h.to_str().ok());
    let price = pick_price(accept_language);

    let base_url = crate::config::app_url();
    let success_url = format!(
        "{}/get-verified/return?session_id={{CHECKOUT_SESSION_ID}}",
        base_url
    );
    let cancel_url = format!("{}/get-verified?status=canceled", base_url);

    let person_id_str = rid.to_raw_string();
    let session = stripe
        .create_checkout_session(
            &person_id_str,
            &user.email,
            price,
            &success_url,
            &cancel_url,
        )
        .await?;

    // Record pending payment.
    if let Err(e) = DB
        .query("CREATE verification_payment SET person = $pid, stripe_checkout_session_id = $sid, amount_minor = $amount, currency = $currency, status = 'pending'")
        .bind(("pid", rid))
        .bind(("sid", session.id.clone()))
        .bind(("amount", price.amount_minor))
        .bind(("currency", price.currency.to_string()))
        .await
    {
        error!("Failed to record pending verification_payment: {}", e);
    }

    Ok(Redirect::to(&session.url).into_response())
}

#[derive(Deserialize)]
struct CheckoutReturnQuery {
    session_id: Option<String>,
}

async fn checkout_return(
    AuthenticatedUser(user): AuthenticatedUser,
    Query(q): Query<CheckoutReturnQuery>,
) -> Result<Response, Error> {
    let Some(stripe) = StripeService::from_env() else {
        return Err(Error::BadRequest(
            "Paid verification is unavailable.".into(),
        ));
    };
    let Some(session_id) = q.session_id else {
        return Err(Error::BadRequest("Missing session id.".into()));
    };

    let rid = user.record_id()?;

    let session = stripe.retrieve_checkout_session(&session_id).await?;
    if session.payment_status != "paid" {
        return Ok(Redirect::to("/get-verified?status=payment_failed").into_response());
    }

    // Mark payment row as paid + store payment_intent.
    if let Err(e) = DB
        .query("UPDATE verification_payment SET status = 'paid', stripe_payment_intent_id = $pi, updated_at = time::now() WHERE stripe_checkout_session_id = $sid AND person = $pid")
        .bind(("pid", rid.clone()))
        .bind(("sid", session_id.clone()))
        .bind(("pi", session.payment_intent.clone()))
        .await
    {
        error!("Failed to mark verification_payment as paid: {}", e);
    }

    // Create the Identity session and redirect.
    let base_url = crate::config::app_url();
    let return_url = format!("{}/get-verified/done", base_url);
    let person_id_str = rid.to_raw_string();
    let id_session = stripe
        .create_identity_session(&person_id_str, &return_url)
        .await?;

    // Attach the Identity session id to the payment row.
    if let Err(e) = DB
        .query("UPDATE verification_payment SET stripe_identity_session_id = $isid, updated_at = time::now() WHERE stripe_checkout_session_id = $sid AND person = $pid")
        .bind(("pid", rid))
        .bind(("sid", session_id))
        .bind(("isid", id_session.id.clone()))
        .await
    {
        error!("Failed to attach identity session id: {}", e);
    }

    info!(person = %user.id, identity = %id_session.id, "redirecting to Stripe Identity");
    Ok(Redirect::to(&id_session.url).into_response())
}

/// Resume a verification that was paid for but never completed. The user
/// might have closed the browser mid-upload, gotten disconnected, started
/// on desktop and now wants mobile (scan QR), or hit a transient error
/// when the Identity session was first created.
///
/// Strategy:
///   1. Find the user's most recent `paid` (not yet `verified`) payment.
///   2. If it has an Identity session id, retrieve from Stripe and
///      redirect to its hosted URL — same session preserves the QR code,
///      partial uploads, and Stripe's internal state.
///   3. If the retrieved session is `canceled`/`redacted`/expired, or if
///      we never created one (the transient-failure case), create a fresh
///      Identity session against the same payment so the user doesn't
///      pay again. Update the payment row with the new session id.
async fn resume_verification(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    let rid = user.record_id()?;

    if is_identity_verified(&rid).await {
        return Ok(Redirect::to("/get-verified/done").into_response());
    }

    let Some(stripe) = StripeService::from_env() else {
        return Err(Error::BadRequest(
            "Paid verification is unavailable.".into(),
        ));
    };

    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        id: surrealdb::types::RecordId,
        stripe_identity_session_id: Option<String>,
        // SurrealDB v3 requires ORDER BY fields to appear in SELECT; this
        // field is unused in Rust but needed so the parser is happy.
        #[allow(dead_code)]
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let mut response = DB
        .query(
            "SELECT id, stripe_identity_session_id, created_at FROM verification_payment \
             WHERE person = $pid AND status = 'paid' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(("pid", rid.clone()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let row: Option<Row> = response
        .take(0)
        .map_err(|e| Error::Database(e.to_string()))?;

    let Some(row) = row else {
        // No paid payment to resume — send them back to start.
        return Ok(Redirect::to("/get-verified").into_response());
    };

    // Path 1: reuse an existing live Stripe Identity session.
    if let Some(isid) = row.stripe_identity_session_id.clone() {
        match stripe.retrieve_identity_session(&isid).await {
            Ok(session) => {
                match session.status.as_str() {
                    "verified" => {
                        info!(
                            person = %user.id,
                            "resume: identity already verified on Stripe side"
                        );
                        return Ok(Redirect::to("/get-verified/done").into_response());
                    }
                    "canceled" | "redacted" => {
                        // Session is terminal — fall through to make a new one.
                        info!(
                            person = %user.id,
                            identity = %isid,
                            status = %session.status,
                            "resume: previous Identity session terminal, creating fresh"
                        );
                    }
                    _ if session.url.is_some() => {
                        // Still live (requires_input / processing / etc.) — send the
                        // user back to the same URL. Stripe preserves their progress,
                        // QR code, scan-to-mobile link, partial uploads.
                        let url = session.url.unwrap();
                        info!(
                            person = %user.id,
                            identity = %isid,
                            status = %session.status,
                            "resume: redirecting to existing Identity session"
                        );
                        return Ok(Redirect::to(&url).into_response());
                    }
                    _ => {
                        // Session exists but has no usable URL — make a new one.
                        warn!(
                            person = %user.id,
                            identity = %isid,
                            status = %session.status,
                            "resume: existing Identity session has no URL, creating fresh"
                        );
                    }
                }
            }
            Err(e) => {
                // Retrieve failed (deleted? bad id?) — fall through to create new.
                warn!(
                    person = %user.id,
                    identity = %isid,
                    error = %e,
                    "resume: retrieve failed, creating fresh Identity session"
                );
            }
        }
    }

    // Path 2: create a fresh Identity session against the existing paid row.
    let base_url = crate::config::app_url();
    let return_url = format!("{}/get-verified/done", base_url);
    let person_id_str = rid.to_raw_string();
    let id_session = stripe
        .create_identity_session(&person_id_str, &return_url)
        .await?;

    if let Err(e) = DB
        .query("UPDATE $id SET stripe_identity_session_id = $isid, updated_at = time::now()")
        .bind(("id", row.id))
        .bind(("isid", id_session.id.clone()))
        .await
    {
        error!("Failed to attach new identity session id on resume: {}", e);
    }

    info!(
        person = %user.id,
        identity = %id_session.id,
        "resume: created new Identity session against existing payment"
    );
    Ok(Redirect::to(&id_session.url).into_response())
}

async fn done_page(AuthenticatedUser(user): AuthenticatedUser) -> Result<Response, Error> {
    let rid = user.record_id()?;
    let verified = is_identity_verified(&rid).await;
    let last_status = latest_payment_status(&rid).await;

    let state = if verified {
        "verified"
    } else if matches!(last_status.as_deref(), Some("failed") | Some("refunded")) {
        "failed"
    } else {
        "processing"
    };

    let base = BaseContext::new()
        .with_page("get-verified")
        .with_user(User::from_session_user(&user).await);

    let template = crate::with_base!(GetVerifiedDoneTemplate, base, {
        state: state.to_string(),
    });

    let html = template.render().map_err(|e| {
        warn!("Failed to render get-verified done template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}
