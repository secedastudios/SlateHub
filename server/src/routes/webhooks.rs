//! Incoming webhook handlers (Stripe today; other providers later).

use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::{error, info, warn};

use crate::{
    db::DB,
    error::Error,
    services::stripe::{RefundReason, StripeService, WebhookEvent},
};

pub fn router() -> Router {
    Router::new().route("/webhooks/stripe", post(stripe_webhook))
}

async fn stripe_webhook(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    let Some(stripe) = StripeService::from_env() else {
        warn!("stripe webhook hit but stripe env is not configured");
        return (StatusCode::SERVICE_UNAVAILABLE, "stripe not configured");
    };

    let signature = match headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
    {
        Some(s) => s,
        None => {
            warn!("stripe webhook missing signature header");
            return (StatusCode::BAD_REQUEST, "missing signature");
        }
    };

    if let Err(e) = stripe.verify_webhook(&body, signature) {
        warn!(error = %e, "stripe webhook signature verification failed");
        return (StatusCode::BAD_REQUEST, "bad signature");
    }

    let event: WebhookEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(e) => {
            error!(error = %e, "stripe webhook payload parse error");
            return (StatusCode::BAD_REQUEST, "bad json");
        }
    };

    info!(event = %event.event_type, id = %event.id, "stripe webhook received");

    // Dispatch — we deliberately ack 200 even on internal processing errors
    // unless the payload itself was invalid. Stripe will retry on 5xx but
    // also on 4xx — we don't want infinite retry storms on programmer errors.
    if let Err(e) = handle_event(&stripe, &event).await {
        error!(event = %event.event_type, id = %event.id, error = %e, "stripe webhook handler error");
    }

    (StatusCode::OK, "ok")
}

async fn handle_event(stripe: &StripeService, event: &WebhookEvent) -> Result<(), Error> {
    match event.event_type.as_str() {
        "checkout.session.completed" => on_checkout_completed(event).await,
        "identity.verification_session.processing" => on_identity_processing(event).await,
        "identity.verification_session.verified" => on_identity_verified(event).await,
        "identity.verification_session.requires_input" => on_identity_requires_input(event).await,
        "identity.verification_session.canceled" => on_identity_canceled(stripe, event).await,
        "charge.refunded" => on_charge_refunded(event).await,
        _ => Ok(()),
    }
}

async fn on_checkout_completed(event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let sid = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::BadRequest("missing checkout session id".into()))?;
    let payment_intent = obj.get("payment_intent").and_then(|v| v.as_str());
    let paid = obj.get("payment_status").and_then(|v| v.as_str()) == Some("paid");

    if !paid {
        return Ok(());
    }

    DB.query(
        "UPDATE verification_payment SET status = 'paid', stripe_payment_intent_id = $pi, updated_at = time::now() \
         WHERE stripe_checkout_session_id = $sid AND status = 'pending'",
    )
    .bind(("sid", sid.to_string()))
    .bind(("pi", payment_intent.map(|s| s.to_string())))
    .await
    .map_err(|e| Error::Database(e.to_string()))?;
    info!(checkout_session = %sid, "verification_payment marked paid");
    Ok(())
}

/// Stripe started reviewing the uploaded documents. Flip the payment row
/// to `processing` so the admin UI and "Almost there" page can show an
/// accurate state. We deliberately do NOT transition out of `verified`
/// (a late-arriving `processing` event after `verified` should be a no-op).
async fn on_identity_processing(event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let isid = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::BadRequest("missing identity session id".into()))?;

    DB.query(
        "UPDATE verification_payment SET status = 'processing', updated_at = time::now() \
         WHERE stripe_identity_session_id = $isid AND status IN ['paid']",
    )
    .bind(("isid", isid.to_string()))
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    info!(identity_session = %isid, "verification_payment marked processing");
    Ok(())
}

async fn on_identity_verified(event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let isid = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::BadRequest("missing identity session id".into()))?;

    // Look up the payment + person.
    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        person: RecordId,
    }
    let mut response = DB
        .query(
            "SELECT person FROM verification_payment WHERE stripe_identity_session_id = $isid LIMIT 1",
        )
        .bind(("isid", isid.to_string()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let row: Option<Row> = response
        .take(0)
        .map_err(|e| Error::Database(e.to_string()))?;
    let Some(row) = row else {
        warn!(identity_session = %isid, "no verification_payment row for verified identity session");
        return Ok(());
    };

    // Atomically: mark payment verified + flip person.verification_status.
    DB.query(
        "BEGIN TRANSACTION;
         UPDATE verification_payment SET status = 'verified', updated_at = time::now() WHERE stripe_identity_session_id = $isid;
         UPDATE $pid SET verification_status = 'identity';
         COMMIT TRANSACTION;",
    )
    .bind(("isid", isid.to_string()))
    .bind(("pid", row.person.clone()))
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    info!(person = ?row.person, identity_session = %isid, "identity verified");

    // Best-effort: in-app notification.
    let person_id_str = format!("{}:{}", row.person.table, person_key(&row.person));
    let _ = crate::models::notification::NotificationModel::new()
        .create(
            &person_id_str,
            "general",
            "You're verified!",
            "Your identity verification was approved. You can now start direct conversations with anyone on SlateHub.",
            Some("/account"),
            None,
        )
        .await;

    Ok(())
}

async fn on_identity_requires_input(event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let isid = obj.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let reason = obj
        .get("last_error")
        .and_then(|e| e.get("reason"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Err(e) = DB
        .query(
            "UPDATE verification_payment SET failure_reason = $r, updated_at = time::now() WHERE stripe_identity_session_id = $isid",
        )
        .bind(("isid", isid.to_string()))
        .bind(("r", reason.clone()))
        .await
    {
        warn!(error = %e, "failed to record identity failure_reason");
    }

    info!(identity_session = %isid, reason = ?reason, "identity verification requires input");
    Ok(())
}

async fn on_identity_canceled(stripe: &StripeService, event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let isid = obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::BadRequest("missing identity session id".into()))?;

    #[derive(serde::Deserialize, SurrealValue)]
    struct Row {
        stripe_payment_intent_id: Option<String>,
    }
    let mut response = DB
        .query("SELECT stripe_payment_intent_id FROM verification_payment WHERE stripe_identity_session_id = $isid LIMIT 1")
        .bind(("isid", isid.to_string()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    let row: Option<Row> = response
        .take(0)
        .map_err(|e| Error::Database(e.to_string()))?;

    if let Some(Row {
        stripe_payment_intent_id: Some(pi),
    }) = row
    {
        match stripe.refund(&pi, RefundReason::RequestedByCustomer).await {
            Ok(refund) => {
                if let Err(e) = DB
                    .query("UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() WHERE stripe_identity_session_id = $isid")
                    .bind(("isid", isid.to_string()))
                    .bind(("rid", refund.id.clone()))
                    .await
                {
                    error!(error = %e, "failed to record refund");
                }
            }
            Err(e) => warn!(error = %e, "refund failed; admin may need to intervene"),
        }
    } else {
        warn!(identity_session = %isid, "no payment intent for canceled identity session; cannot auto-refund");
    }
    Ok(())
}

/// Sync dashboard-initiated refunds back to our DB. When an admin issues a
/// refund directly in Stripe (rather than via our /admin/payments refund
/// button or the auto-refund job), Stripe still fires `charge.refunded` —
/// this handler is the listener that keeps our state consistent.
///
/// The event's `data.object` is the **charge**, which carries
/// `payment_intent` (matches our `stripe_payment_intent_id`) and a `refunds`
/// sub-collection. We grab the first refund id for our records. We do NOT
/// flip a row that's already `refunded` — `webhook events can arrive twice
/// (Stripe retries).
async fn on_charge_refunded(event: &WebhookEvent) -> Result<(), Error> {
    let obj = &event.data.object;
    let payment_intent = obj
        .get("payment_intent")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::BadRequest("missing payment_intent on charge".into()))?;

    // The first (and usually only) refund id for this charge.
    let refund_id = obj
        .get("refunds")
        .and_then(|r| r.get("data"))
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string());

    DB.query(
        "UPDATE verification_payment SET status = 'refunded', refund_id = $rid, updated_at = time::now() \
         WHERE stripe_payment_intent_id = $pi AND status != 'refunded'",
    )
    .bind(("pi", payment_intent.to_string()))
    .bind(("rid", refund_id.clone()))
    .await
    .map_err(|e| Error::Database(e.to_string()))?;

    info!(
        payment_intent = %payment_intent,
        refund_id = ?refund_id,
        "verification_payment synced from charge.refunded"
    );
    Ok(())
}

fn person_key(rid: &RecordId) -> String {
    use crate::record_id_ext::RecordIdExt;
    rid.key_string()
}
