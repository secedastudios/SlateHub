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
    AuthenticatedUser(_user): AuthenticatedUser,
    Form(form): Form<MarkReadForm>,
) -> Result<Redirect, Error> {
    debug!("Marking notification as read: {}", form.notification_id);

    let notification_model = NotificationModel::new();
    notification_model.mark_read(&form.notification_id).await?;

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

    // Delete the notification
    notification_model.delete(&form.notification_id).await?;

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

    // Delete the notification
    notification_model.delete(&form.notification_id).await?;

    Ok(Redirect::to("/notifications"))
}

async fn delete_notification(
    AuthenticatedUser(_user): AuthenticatedUser,
    Form(form): Form<MarkReadForm>,
) -> Result<Redirect, Error> {
    debug!("Deleting notification: {}", form.notification_id);

    let notification_model = NotificationModel::new();
    notification_model.delete(&form.notification_id).await?;

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
