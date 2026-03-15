use askama::Template;
use axum::{
    Form, Router,
    extract::Path,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{debug, error};

use crate::{
    error::Error,
    middleware::AuthenticatedUser,
    models::{
        messaging::MessagingModel,
        notification::NotificationModel,
        person::Person,
    },
    record_id_ext::RecordIdExt,
    services::email::EmailService,
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

// -- View structs for templates --

struct ConversationView {
    id: String,
    other_person_name: String,
    other_person_username: String,
    other_person_avatar: Option<String>,
    other_person_initials: String,
    last_message_at: String,
    unread_count: u32,
}

struct MessageView {
    body: String,
    is_own: bool,
    created_at: String,
}

// -- Templates --

#[derive(Template)]
#[template(path = "messages/inbox.html")]
struct InboxTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    conversations: Vec<ConversationView>,
}

#[derive(Template)]
#[template(path = "messages/conversation.html")]
struct ConversationTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    conversation_id: String,
    other_person_name: String,
    other_person_username: String,
    other_person_avatar: Option<String>,
    other_person_initials: String,
    messages: Vec<MessageView>,
}

#[derive(Template)]
#[template(path = "messages/new.html")]
struct NewMessageTemplate {
    app_name: String,
    year: i32,
    version: String,
    active_page: String,
    user: Option<User>,
    recipient_username: String,
    recipient_name: String,
    recipient_avatar: Option<String>,
    recipient_initials: String,
    error: Option<String>,
}

pub fn router() -> Router {
    Router::new()
        .route("/messages", get(inbox))
        .route("/messages/new/{username}", get(new_message_page))
        .route("/messages/send", post(send_message))
        .route("/messages/{conversation_id}", get(view_conversation))
        .route("/messages/{conversation_id}/reply", post(reply_message))
}

// -- Handlers --

async fn inbox(
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Html<String>, Error> {
    debug!("Listing conversations for user: {}", user.id);

    let model = MessagingModel::new();
    let conversations = model.get_conversations(&user.id).await?;

    let mut views = Vec::new();
    for conv in &conversations {
        let other_id = MessagingModel::get_other_participant(conv, &user.id);
        let other_person = Person::find_by_id(&other_id).await?.unwrap_or_else(|| {
            Person {
                id: surrealdb::types::RecordId::parse_simple(&other_id)
                    .unwrap_or_else(|_| surrealdb::types::RecordId::new("person", "unknown")),
                username: "deleted".to_string(),
                email: String::new(),
                name: Some("Deleted User".to_string()),
                verification_status: "unverified".to_string(),
                profile: None,
                messaging_preference: "nobody".to_string(),
            }
        });

        // Count unread messages in this conversation
        let conv_id = conv.id.to_raw_string();
        let msgs = model.get_messages(&conv_id, 500).await.unwrap_or_default();
        let unread = msgs
            .iter()
            .filter(|m| m.sender.to_raw_string() != user.id && !m.read)
            .count() as u32;

        views.push(ConversationView {
            id: conv_id,
            other_person_name: other_person.get_display_name(),
            other_person_username: other_person.username.clone(),
            other_person_avatar: other_person.get_avatar_url(),
            other_person_initials: other_person.get_initials(),
            last_message_at: conv.last_message_at.format("%b %d, %Y %H:%M").to_string(),
            unread_count: unread,
        });
    }

    let base = BaseContext::new()
        .with_page("messages")
        .with_user(User::from_session_user(&user).await);

    let template = InboxTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        conversations: views,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render inbox template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn view_conversation(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(conversation_id): Path<String>,
) -> Result<Html<String>, Error> {
    debug!("Viewing conversation: {}", conversation_id);

    let model = MessagingModel::new();

    // Verify the user is a participant in this conversation
    let conversations = model.get_conversations(&user.id).await?;
    let conv = conversations
        .iter()
        .find(|c| c.id.to_raw_string() == conversation_id)
        .ok_or(Error::NotFound)?;

    // Mark messages as read
    model
        .mark_conversation_read(&conversation_id, &user.id)
        .await?;

    // Get the other participant
    let other_id = MessagingModel::get_other_participant(conv, &user.id);
    let other_person = Person::find_by_id(&other_id)
        .await?
        .ok_or(Error::NotFound)?;

    // Get messages
    let raw_messages = model.get_messages(&conversation_id, 200).await?;
    let messages: Vec<MessageView> = raw_messages
        .into_iter()
        .map(|m| {
            let is_own = m.sender.to_raw_string() == user.id;
            MessageView {
                body: m.body,
                is_own,
                created_at: m.created_at.format("%b %d, %H:%M").to_string(),
            }
        })
        .collect();

    let base = BaseContext::new()
        .with_page("messages")
        .with_user(User::from_session_user(&user).await);

    let template = ConversationTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        conversation_id,
        other_person_name: other_person.get_display_name(),
        other_person_username: other_person.username.clone(),
        other_person_avatar: other_person.get_avatar_url(),
        other_person_initials: other_person.get_initials(),
        messages,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render conversation template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

async fn new_message_page(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(username): Path<String>,
) -> Result<Response, Error> {
    debug!("New message to: {}", username);

    let recipient = Person::find_by_username(&username)
        .await?
        .ok_or(Error::NotFound)?;

    // Check if there's already a conversation — if so, redirect to it
    let model = MessagingModel::new();
    let recipient_id = recipient.id.to_raw_string();
    let conversations = model.get_conversations(&user.id).await?;
    for conv in &conversations {
        let other_id = MessagingModel::get_other_participant(conv, &user.id);
        if other_id == recipient_id {
            return Ok(Redirect::to(&format!("/messages/{}", conv.id.to_raw_string()))
                .into_response());
        }
    }

    // Check messaging preference
    let error = check_messaging_preference(&recipient, &user.id).await;

    let base = BaseContext::new()
        .with_page("messages")
        .with_user(User::from_session_user(&user).await);

    let template = NewMessageTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        recipient_username: recipient.username.clone(),
        recipient_name: recipient.get_display_name(),
        recipient_avatar: recipient.get_avatar_url(),
        recipient_initials: recipient.get_initials(),
        error,
    };

    let html = template.render().map_err(|e| {
        error!("Failed to render new message template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

#[derive(Debug, Deserialize)]
struct SendMessageForm {
    recipient_username: String,
    body: String,
}

async fn send_message(
    AuthenticatedUser(user): AuthenticatedUser,
    Form(form): Form<SendMessageForm>,
) -> Result<Redirect, Error> {
    let body = form.body.trim();
    if body.is_empty() {
        return Err(Error::BadRequest("Message cannot be empty.".to_string()));
    }
    if body.len() > 5000 {
        return Err(Error::BadRequest(
            "Message is too long (max 5000 characters).".to_string(),
        ));
    }

    let recipient = Person::find_by_username(&form.recipient_username)
        .await?
        .ok_or(Error::NotFound)?;

    // Check messaging preference
    if let Some(err) = check_messaging_preference(&recipient, &user.id).await {
        return Err(Error::BadRequest(err));
    }

    let recipient_id = recipient.id.to_raw_string();
    let model = MessagingModel::new();
    let conv = model
        .get_or_create_conversation(&user.id, &recipient_id)
        .await?;
    let conv_id = conv.id.to_raw_string();

    let sanitized_body = ammonia::clean(body);
    model
        .send_message(&conv_id, &user.id, &sanitized_body)
        .await?;

    // Create notification and send email
    send_new_message_notification(&user.id, &user.username, &recipient, &conv_id, &sanitized_body)
        .await;

    Ok(Redirect::to(&format!("/messages/{}", conv_id)))
}

#[derive(Debug, Deserialize)]
struct ReplyForm {
    body: String,
}

async fn reply_message(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(conversation_id): Path<String>,
    Form(form): Form<ReplyForm>,
) -> Result<Redirect, Error> {
    let body = form.body.trim();
    if body.is_empty() {
        return Err(Error::BadRequest("Message cannot be empty.".to_string()));
    }
    if body.len() > 5000 {
        return Err(Error::BadRequest(
            "Message is too long (max 5000 characters).".to_string(),
        ));
    }

    let model = MessagingModel::new();

    // Verify the user is a participant
    let conversations = model.get_conversations(&user.id).await?;
    let conv = conversations
        .iter()
        .find(|c| c.id.to_raw_string() == conversation_id)
        .ok_or(Error::NotFound)?;

    let sanitized_body = ammonia::clean(body);
    model
        .send_message(&conversation_id, &user.id, &sanitized_body)
        .await?;

    // Notify the other participant
    let other_id = MessagingModel::get_other_participant(conv, &user.id);
    if let Ok(Some(recipient)) = Person::find_by_id(&other_id).await {
        send_new_message_notification(
            &user.id,
            &user.username,
            &recipient,
            &conversation_id,
            &sanitized_body,
        )
        .await;
    }

    Ok(Redirect::to(&format!("/messages/{}", conversation_id)))
}

// -- Helpers --

/// Check if the current user can message the recipient based on their messaging_preference.
/// Returns None if allowed, Some(error_message) if not.
async fn check_messaging_preference(recipient: &Person, sender_id: &str) -> Option<String> {
    if recipient.id.to_raw_string() == sender_id {
        return Some("You cannot message yourself.".to_string());
    }

    match recipient.messaging_preference.as_str() {
        "nobody" => Some(format!(
            "{} is not accepting messages.",
            recipient.get_display_name()
        )),
        "verified" => {
            if let Ok(Some(sender)) = Person::find_by_id(sender_id).await {
                if sender.verification_status == "identity" {
                    None
                } else {
                    Some(format!(
                        "{} only accepts messages from verified accounts. Get verified to send them a message.",
                        recipient.get_display_name()
                    ))
                }
            } else {
                Some("Unable to verify your account status.".to_string())
            }
        }
        _ => None, // "anyone"
    }
}

/// Create a notification and send an email for a new message.
async fn send_new_message_notification(
    sender_id: &str,
    sender_username: &str,
    recipient: &Person,
    conversation_id: &str,
    message_body: &str,
) {
    let sender_person = Person::find_by_id(sender_id).await.ok().flatten();
    let sender_name = sender_person
        .map(|p| p.get_display_name())
        .unwrap_or_else(|| sender_username.to_string());

    let recipient_id = recipient.id.to_raw_string();
    let body_preview = truncate_body(message_body, 100);

    // Create in-app notification
    let notification_model = NotificationModel::new();
    let _ = notification_model
        .create(
            &recipient_id,
            "message",
            &format!("New message from {}", sender_name),
            &body_preview,
            Some(&format!("/messages/{}", conversation_id)),
            Some(conversation_id),
        )
        .await;

    // Send email notification asynchronously
    let recipient_email = recipient.email.clone();
    let recipient_name = recipient.get_display_name();
    let sender_name_clone = sender_name.clone();
    let body_preview_long = truncate_body(message_body, 200);
    let conv_id = conversation_id.to_string();
    tokio::spawn(async move {
        if let Ok(email_service) = EmailService::from_env() {
            let base_url = crate::config::app_url();
            let message_url = format!("{}/messages/{}", base_url, conv_id);
            let subject = format!("New message from {} on SlateHub", sender_name_clone);

            let text_body = format!(
                "Hi {},\n\n{} sent you a message on SlateHub:\n\n\"{}\"\n\nView and reply: {}\n\nBest regards,\nThe SlateHub Team",
                recipient_name, sender_name_clone, body_preview_long, message_url
            );

            let html_body = format!(
                r#"<!DOCTYPE html>
<html>
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #171717; border-radius: 8px; padding: 30px; margin-bottom: 20px;">
        <h1 style="color: #d6d8ca; margin-top: 0;">New Message</h1>
        <p style="font-size: 16px; color: #d6d8ca;">{} sent you a message on SlateHub.</p>
    </div>
    <div style="background-color: #ffffff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 30px;">
        <div style="background-color: #f5f5f5; border-left: 3px solid #eb5437; padding: 15px 20px; margin: 20px 0; border-radius: 4px;">
            <p style="font-size: 15px; color: #333; margin: 0; font-style: italic;">"{}"</p>
        </div>
        <div style="text-align: center; margin: 30px 0;">
            <a href="{}" style="display: inline-block; background-color: #eb5437; color: white; padding: 14px 36px; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">View Message</a>
        </div>
    </div>
    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>&copy; 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
                sender_name_clone, body_preview_long, message_url
            );

            if let Err(e) = email_service
                .send_notification_email(
                    &recipient_email,
                    Some(&recipient_name),
                    &subject,
                    &text_body,
                    &html_body,
                )
                .await
            {
                error!("Failed to send message notification email: {}", e);
            }
        }
    });
}

fn truncate_body(body: &str, max_len: usize) -> String {
    if body.len() <= max_len {
        body.to_string()
    } else {
        format!("{}...", &body[..max_len])
    }
}
