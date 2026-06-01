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
use tracing::{error, info, warn};

use crate::{
    db::DB,
    error::Error,
    middleware::{AuthenticatedUser, UserExtractor},
    services::feature_flag,
    services::stripe::{StripeService, pick_price},
    templates::{BaseContext, GetVerifiedDoneTemplate, GetVerifiedTemplate, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/get-verified", get(get_verified_page))
        .route("/get-verified/request", post(request_verification))
        .route("/get-verified/checkout", post(start_checkout))
        .route("/get-verified/return", get(checkout_return))
        .route("/get-verified/done", get(done_page))
}

fn parse_person_rid(person_id: &str) -> Option<RecordId> {
    if person_id.starts_with("person:") {
        RecordId::parse_simple(person_id).ok()
    } else {
        Some(RecordId::new("person", person_id))
    }
}

async fn has_pending_verification(person_id: &str) -> bool {
    let Some(rid) = parse_person_rid(person_id) else {
        return false;
    };
    if let Ok(mut result) = DB
        .query("SELECT count() AS c FROM verification_request WHERE person = $pid AND status = 'pending' GROUP ALL")
        .bind(("pid", rid))
        .await
        && let Ok(Some(row)) = result.take::<Option<serde_json::Value>>(0)
            && let Some(c) = row.get("c").and_then(|v| v.as_i64()) {
                return c > 0;
            }
    false
}

/// Most recent verification_payment status for this person, if any.
async fn latest_payment_status(person_id: &str) -> Option<String> {
    let rid = parse_person_rid(person_id)?;
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        status: String,
    }
    let mut response = DB
        .query("SELECT status FROM verification_payment WHERE person = $pid ORDER BY created_at DESC LIMIT 1")
        .bind(("pid", rid))
        .await
        .ok()?;
    let row: Option<Row> = response.take(0).ok().flatten();
    row.map(|r| r.status)
}

async fn is_identity_verified(person_id: &str) -> bool {
    let Some(rid) = parse_person_rid(person_id) else {
        return false;
    };
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        verification_status: String,
    }
    let mut response = match DB
        .query("SELECT verification_status FROM $pid")
        .bind(("pid", rid))
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
        pending = has_pending_verification(&user.id).await;
        last_status = latest_payment_status(&user.id).await;
        already_verified = is_identity_verified(&user.id).await;
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

    let template = GetVerifiedTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        has_pending_request: pending,
        already_verified,
        paid_flow_enabled,
        price_label: price.label.to_string(),
        last_payment_status: last_status,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render get-verified template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

async fn request_verification(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, Error> {
    let person_id = &user.id;

    if has_pending_verification(person_id).await {
        return Ok(Redirect::to("/get-verified").into_response());
    }

    let rid = parse_person_rid(person_id)
        .ok_or_else(|| Error::BadRequest("Invalid person ID".to_string()))?;

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
    if is_identity_verified(&user.id).await {
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

    let session = stripe
        .create_checkout_session(&user.id, &user.email, price, &success_url, &cancel_url)
        .await?;

    // Record pending payment.
    let rid = parse_person_rid(&user.id)
        .ok_or_else(|| Error::BadRequest("Invalid person ID".to_string()))?;
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

    let session = stripe.retrieve_checkout_session(&session_id).await?;
    if session.payment_status != "paid" {
        return Ok(Redirect::to("/get-verified?status=payment_failed").into_response());
    }

    // Mark payment row as paid + store payment_intent.
    let rid = parse_person_rid(&user.id)
        .ok_or_else(|| Error::BadRequest("Invalid person ID".to_string()))?;
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
    let id_session = stripe
        .create_identity_session(&user.id, &return_url)
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

async fn done_page(AuthenticatedUser(user): AuthenticatedUser) -> Result<Response, Error> {
    let verified = is_identity_verified(&user.id).await;
    let last_status = latest_payment_status(&user.id).await;

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

    let template = GetVerifiedDoneTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        state: state.to_string(),
    };

    let html = template.render().map_err(|e| {
        warn!("Failed to render get-verified done template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}
