use askama::Template;
use axum::{
    Form, Router,
    response::{Html, Redirect},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::AuthenticatedUser,
    models::{
        membership::MembershipModel,
        notification::NotificationModel,
    },
    record_id_ext::RecordIdExt,
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

/// Template-friendly notification view with String fields instead of RecordId
struct NotificationView {
    id: String,
    notification_type: String,
    title: String,
    message: String,
    link: Option<String>,
    read: bool,
    related_id: Option<String>,
    created_at: String,
}

#[derive(Template)]
#[template(path = "notifications/index.html")]
struct NotificationsTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    notifications: Vec<NotificationView>,
}

impl NotificationsTemplate {
    fn new(base: BaseContext, notifications: Vec<NotificationView>) -> Self {
        Self {
            app_name: base.app_name,
            year: base.year,
            version: base.version,
            active_page: base.active_page,
            user: base.user,
            notifications,
        }
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/notifications", get(list_notifications))
        .route("/api/notifications/stream", get(notification_stream_sse))
        .route("/notifications/mark-read", post(mark_read))
        .route("/notifications/read-all", post(mark_all_read))
        .route("/notifications/delete", post(delete_notification))
        .route("/notifications/clear-all", post(clear_all_notifications))
        .route("/invitations/accept", post(accept_invitation))
        .route("/invitations/decline", post(decline_invitation))
}

async fn list_notifications(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Listing notifications for user: {}", user.id);

    let notification_model = NotificationModel::new();
    let raw_notifications = notification_model.get_recent(&user.id, 50).await?;

    let notifications: Vec<NotificationView> = raw_notifications
        .into_iter()
        .map(|n| NotificationView {
            id: n.id.to_raw_string(),
            notification_type: n.notification_type,
            title: n.title,
            message: n.message,
            link: n.link,
            read: n.read,
            related_id: n.related_id,
            created_at: n.created_at.format("%b %d, %Y at %H:%M").to_string(),
        })
        .collect();

    let base = BaseContext::new()
        .with_page("notifications")
        .with_user(User::from_session_user(&user).await);

    let template = NotificationsTemplate::new(base, notifications);

    let html = template.render().map_err(|e| {
        error!("Failed to render notifications template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
struct MarkReadForm {
    notification_id: String,
}

async fn mark_read(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(form): Form<MarkReadForm>,
) -> Result<Redirect, Error> {
    debug!("Marking notification as read: {}", form.notification_id);

    let notification_model = NotificationModel::new();
    notification_model.mark_read(&form.notification_id, &user.id).await?;

    Ok(Redirect::to("/notifications"))
}

async fn mark_all_read(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Redirect, Error> {
    debug!("Marking all notifications as read for user: {}", user.id);

    let notification_model = NotificationModel::new();
    notification_model.mark_all_read(&user.id).await?;

    Ok(Redirect::to("/notifications"))
}

#[derive(Debug, Deserialize)]
struct InvitationActionForm {
    org_id: String,
    notification_id: String,
}

async fn accept_invitation(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(form): Form<InvitationActionForm>,
) -> Result<Redirect, Error> {
    debug!("User {} accepting invitation to org {}", user.id, form.org_id);

    let membership_model = MembershipModel::new();
    let notification_model = NotificationModel::new();

    // Find the membership for this user + org
    if let Some(membership) = membership_model.find_by_person_and_org(&user.id, &form.org_id).await? {
        let membership_id = membership.id.to_raw_string();
        membership_model.accept_invitation(&membership_id).await?;
        info!("User {} accepted invitation to org {}", user.id, form.org_id);
    }

    // Delete the notification (scoped to this user)
    notification_model.delete(&form.notification_id, &user.id).await?;

    // Redirect to the org
    let org_slug = get_org_slug(&form.org_id).await;
    Ok(Redirect::to(&format!("/orgs/{}", org_slug.unwrap_or_else(|| "".to_string()))))
}

async fn decline_invitation(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(form): Form<InvitationActionForm>,
) -> Result<Redirect, Error> {
    debug!("User {} declining invitation to org {}", user.id, form.org_id);

    let membership_model = MembershipModel::new();
    let notification_model = NotificationModel::new();

    // Find the membership for this user + org
    if let Some(membership) = membership_model.find_by_person_and_org(&user.id, &form.org_id).await? {
        let membership_id = membership.id.to_raw_string();
        membership_model.decline_invitation(&membership_id).await?;
        info!("User {} declined invitation to org {}", user.id, form.org_id);
    }

    // Delete the notification (scoped to this user)
    notification_model.delete(&form.notification_id, &user.id).await?;

    Ok(Redirect::to("/notifications"))
}

async fn delete_notification(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(form): Form<MarkReadForm>,
) -> Result<Redirect, Error> {
    debug!("Deleting notification: {}", form.notification_id);

    let notification_model = NotificationModel::new();
    notification_model.delete(&form.notification_id, &user.id).await?;

    Ok(Redirect::to("/notifications"))
}

async fn clear_all_notifications(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Redirect, Error> {
    debug!("Clearing all notifications for user: {}", user.id);

    let notification_model = NotificationModel::new();
    notification_model.delete_all(&user.id).await?;

    Ok(Redirect::to("/notifications"))
}

async fn get_org_slug(org_id: &str) -> Option<String> {
    use crate::models::organization::OrganizationModel;
    let model = OrganizationModel::new();
    model.get_by_id(org_id).await.ok().map(|org| org.slug)
}

/// SSE endpoint that pushes notification count updates to the authenticated user.
/// The person_id is derived from the JWT — never from URL params.
async fn notification_stream_sse(
    request: axum::extract::Request,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::header;
    use crate::middleware::UserExtractor;

    // Silently return empty stream if not authenticated
    let person_id = match request.get_user() {
        Some(user) => user.id.clone(),
        None => {
            let body = Body::from_stream(async_stream::stream! {
                yield Ok::<_, std::convert::Infallible>(":unauthenticated\n\n".to_string());
            });
            return axum::response::Response::builder()
                .header(header::CONTENT_TYPE, "text/event-stream")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(body)
                .unwrap();
        }
    };
    let mut rx = crate::services::notification_stream::subscribe();

    let stream = async_stream::stream! {
        // Send initial count immediately
        let notification_model = NotificationModel::new();
        if let Ok(count) = notification_model.get_unread_count(&person_id).await {
            yield Ok::<_, std::convert::Infallible>(
                sse_notification_event(count)
            );
        }

        // Keep-alive comment every 30s to prevent timeout
        let mut keepalive = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            if event.person_id == person_id {
                                let notification_model = NotificationModel::new();
                                if let Ok(count) = notification_model.get_unread_count(&person_id).await {
                                    yield Ok(sse_notification_event(count));
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let notification_model = NotificationModel::new();
                            if let Ok(count) = notification_model.get_unread_count(&person_id).await {
                                yield Ok(sse_notification_event(count));
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = keepalive.tick() => {
                    yield Ok(":keepalive\n\n".to_string());
                }
            }
        }
    };

    let body = Body::from_stream(stream);

    axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap()
}

fn sse_notification_event(count: u32) -> String {
    let badge = if count > 0 {
        format!(
            "<span id=\\\"notification-badge\\\" data-role=\\\"notification-badge\\\" aria-label=\\\"{} unread notifications\\\">{}</span>",
            count, count
        )
    } else {
        "<span id=\\\"notification-badge\\\" data-role=\\\"notification-badge\\\" style=\\\"display:none\\\"></span>".to_string()
    };

    let menu_badge = if count > 0 {
        format!("<span data-role=\\\"menu-badge\\\">{}</span>", count)
    } else {
        "<span data-role=\\\"menu-badge\\\" style=\\\"display:none\\\"></span>".to_string()
    };

    format!(
        "event: notification-update\ndata: {{\"badge\":\"{}\",\"menu_badge\":\"{}\"}}\n\n",
        badge, menu_badge
    )
}
