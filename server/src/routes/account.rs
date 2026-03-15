use askama::Template;
use axum::{
    Form, Router,
    extract::Query,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use std::env;
use tracing::{error, info};

use crate::{
    auth,
    db::DB,
    error::Error,
    middleware::AuthenticatedUser,
    models::person::Person,
    record_id_ext::RecordIdExt,
    response,
    templates::{AccountSettingsTemplate, BaseContext, User},
};

pub fn router() -> Router {
    Router::new()
        .route("/account", get(account_settings_page))
        .route("/account/change-password", post(change_password))
        .route("/account/change-username", post(change_username))
        .route("/account/messaging-preference", post(change_messaging_preference))
        .route("/account/delete", post(delete_account))
}

#[derive(Debug, Deserialize)]
struct AccountQuery {
    success: Option<String>,
}

async fn account_settings_page(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(query): Query<AccountQuery>,
) -> Result<Response, Error> {
    let mut base = BaseContext::new().with_page("account");
    base = base.with_user(User::from_session_user(&current_user).await);

    let person = Person::find_by_id(&current_user.id)
        .await?
        .ok_or(Error::NotFound)?;

    let mut template = AccountSettingsTemplate::new(base);
    template.username = person.username;
    template.email = person.email;
    template.messaging_preference = person.messaging_preference;
    template.success = query.success;

    let html = template.render().map_err(|e| {
        error!("Failed to render account settings template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

// -- Change Password --

#[derive(Debug, Deserialize)]
struct ChangePasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

async fn change_password(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<ChangePasswordForm>,
) -> Result<Response, Error> {
    // Validate new passwords match
    if form.new_password != form.confirm_password {
        return render_settings_with_error(&current_user.id, "New passwords do not match.").await;
    }

    // Validate new password length
    if form.new_password.len() < 8 {
        return render_settings_with_error(
            &current_user.id,
            "New password must be at least 8 characters.",
        )
        .await;
    }

    // Verify current password
    let person = Person::authenticate(&current_user.username, &form.current_password)
        .await
        .map_err(|_| Error::BadRequest("Current password is incorrect.".to_string()))?
        .ok_or_else(|| Error::BadRequest("Current password is incorrect.".to_string()))?;

    // Hash and update password
    let password_hash = auth::hash_password(&form.new_password)?;
    let sql = "UPDATE person SET password = $password WHERE id = $id";
    DB.query(sql)
        .bind(("password", password_hash))
        .bind(("id", person.id.clone()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Password changed for user: {}", current_user.username);

    render_settings_with_success(&current_user.id, "Password changed successfully.").await
}

// -- Change Username --

#[derive(Debug, Deserialize)]
struct ChangeUsernameForm {
    new_username: String,
    password: String,
}

async fn change_username(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<ChangeUsernameForm>,
) -> Result<Response, Error> {
    // Verify password
    let person = Person::authenticate(&current_user.username, &form.password)
        .await
        .map_err(|_| Error::BadRequest("Password is incorrect.".to_string()))?
        .ok_or_else(|| Error::BadRequest("Password is incorrect.".to_string()))?;

    // Validate new username
    let new_username = crate::models::person::validate_username(&form.new_username)?;

    if new_username == person.username {
        return render_settings_with_error(
            &current_user.id,
            "New username is the same as your current username.",
        )
        .await;
    }

    // Check if username is taken
    if let Some(_) = Person::find_by_username(&new_username).await? {
        return render_settings_with_error(&current_user.id, "That username is already taken.")
            .await;
    }

    // Update username
    let sql = "UPDATE person SET username = $username WHERE id = $id";
    DB.query(sql)
        .bind(("username", new_username.clone()))
        .bind(("id", person.id.clone()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!(
        "Username changed from {} to {} for user {}",
        person.username,
        new_username,
        person.id.to_raw_string()
    );

    // Issue new JWT with updated username
    let token = auth::create_jwt(
        &person.id.to_raw_string(),
        &new_username,
        &person.email,
    )?;

    let cookie = Cookie::build(("auth_token", token))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
        .build();

    // Redirect back so the new cookie takes effect
    Ok((
        CookieJar::new().add(cookie),
        response::redirect("/account?success=Username+changed+successfully."),
    )
        .into_response())
}

// -- Messaging Preference --

#[derive(Debug, Deserialize)]
struct MessagingPreferenceForm {
    messaging_preference: String,
}

async fn change_messaging_preference(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<MessagingPreferenceForm>,
) -> Result<Response, Error> {
    let pref = form.messaging_preference.as_str();
    if !["nobody", "verified", "anyone"].contains(&pref) {
        return render_settings_with_error(&current_user.id, "Invalid messaging preference.").await;
    }

    let person = Person::find_by_id(&current_user.id)
        .await?
        .ok_or(Error::NotFound)?;

    DB.query("UPDATE $id SET messaging_preference = $pref")
        .bind(("id", person.id.clone()))
        .bind(("pref", pref.to_string()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Messaging preference changed to '{}' for user: {}", pref, current_user.username);

    render_settings_with_success(&current_user.id, "Messaging preference updated.").await
}

// -- Delete Account --

#[derive(Debug, Deserialize)]
struct DeleteAccountForm {
    password: String,
    confirm_delete: Option<String>,
}

async fn delete_account(
    AuthenticatedUser(current_user): AuthenticatedUser,
    Form(form): Form<DeleteAccountForm>,
) -> Result<Response, Error> {
    // Require explicit confirmation
    if form.confirm_delete.as_deref() != Some("DELETE") {
        return render_settings_with_error(
            &current_user.id,
            "You must type DELETE to confirm account deletion.",
        )
        .await;
    }

    // Verify password
    let person = Person::authenticate(&current_user.username, &form.password)
        .await
        .map_err(|_| Error::BadRequest("Password is incorrect.".to_string()))?
        .ok_or_else(|| Error::BadRequest("Password is incorrect.".to_string()))?;

    let person_id_str = person.id.to_raw_string();

    // Delete related data: involvements, notifications, verification codes, org memberships
    let cleanup_sql = "
        DELETE FROM involvement WHERE in = $person_id;
        DELETE FROM notification WHERE person_id = $person_id;
        DELETE FROM verification_code WHERE person_id = $person_id;
        DELETE FROM member_of WHERE in = $person_id;
    ";
    if let Err(e) = DB
        .query(cleanup_sql)
        .bind(("person_id", person.id.clone()))
        .await
    {
        error!(
            "Failed to clean up related data for {}: {}",
            person_id_str, e
        );
    }

    // Delete the person record itself
    let delete_sql = "DELETE FROM person WHERE id = $id";
    DB.query(delete_sql)
        .bind(("id", person.id.clone()))
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    info!("Account deleted: {} ({})", person.username, person_id_str);

    // Clear auth cookie and redirect
    let cookie = Cookie::build(("auth_token", ""))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
        .max_age(Default::default())
        .build();

    Ok((CookieJar::new().remove(cookie), response::redirect("/")).into_response())
}

// -- Helpers --

async fn render_settings_with_error(person_id: &str, error_msg: &str) -> Result<Response, Error> {
    let person = Person::find_by_id(person_id)
        .await?
        .ok_or(Error::NotFound)?;

    let base = BaseContext::new().with_page("account");
    let mut template = AccountSettingsTemplate::new(base);
    template.username = person.username;
    template.email = person.email;
    template.messaging_preference = person.messaging_preference;
    template.error = Some(error_msg.to_string());

    let html = template.render().map_err(|e| {
        error!("Failed to render account settings template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

async fn render_settings_with_success(
    person_id: &str,
    success_msg: &str,
) -> Result<Response, Error> {
    let person = Person::find_by_id(person_id)
        .await?
        .ok_or(Error::NotFound)?;

    let mut base = BaseContext::new().with_page("account");

    // Re-fetch user for header
    let session_user = person.to_session_user();
    base = base.with_user(User::from_session_user(&session_user).await);

    let mut template = AccountSettingsTemplate::new(base);
    template.username = person.username;
    template.email = person.email;
    template.messaging_preference = person.messaging_preference;
    template.success = Some(success_msg.to_string());

    let html = template.render().map_err(|e| {
        error!("Failed to render account settings template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}
