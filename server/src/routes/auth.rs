use askama::Template;
use axum::{
    Form, Router,
    extract::{Query, Request},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use std::env;

use tracing::{debug, error, info};

use crate::{
    error::Error,
    middleware::UserExtractor,
    models::person::{CreateUser, LoginUser, Person},
    response,
    services::{
        email::EmailService,
        verification::{CodeType, VerificationService},
    },
    templates::{
        BaseContext, EmailVerificationTemplate, ForgotPasswordTemplate, LoginTemplate,
        ResetPasswordTemplate, SignupTemplate, User,
    },
};

pub fn router() -> Router {
    Router::new()
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
        .route("/verify-email", get(verify_email_form).post(verify_email))
        .route("/resend-verification", post(resend_verification))
        .route(
            "/forgot-password",
            get(forgot_password_form).post(forgot_password),
        )
        .route(
            "/reset-password",
            get(reset_password_form).post(reset_password),
        )
}

async fn signup_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering signup page");

    let mut base = BaseContext::new().with_page("signup");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = SignupTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn signup(Form(form): Form<CreateUser>) -> Result<Response, Error> {
    debug!("Processing signup for email: {}", form.email);

    // Try to create the user
    match Person::signup(form.username, form.email, form.password).await {
        Ok(token) => {
            info!("User created successfully");

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
                .build();

            // Redirect to email verification page
            Ok((
                CookieJar::new().add(cookie),
                response::redirect("/verify-email"),
            )
                .into_response())
        }
        Err(e) => {
            error!("Signup failed: {}", e);

            // Re-render the signup form with error
            let base = BaseContext::new().with_page("signup");

            let mut template = SignupTemplate::new(base);
            template.error = Some(e.to_string());

            let html = template.render().map_err(|e| {
                error!("Failed to render signup template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

async fn login_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering login page");

    let mut base = BaseContext::new().with_page("login");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = LoginTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render login template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn login(Form(form): Form<LoginUser>) -> Result<Response, Error> {
    debug!("Processing login for: {}", form.email);

    // Try to authenticate the user (signin accepts username or email as identifier)
    match Person::signin(form.email.clone(), form.password).await {
        Ok(token) => {
            info!("User logged in successfully");

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
                .build();

            // Redirect to profile or the originally requested page
            let redirect_to = form.redirect_to.unwrap_or_else(|| "/profile".to_string());

            Ok((
                CookieJar::new().add(cookie),
                response::redirect(&redirect_to),
            )
                .into_response())
        }
        Err(e) => {
            error!("Login failed for {}: {}", form.email, e);

            // Re-render the login form with error
            let base = BaseContext::new().with_page("login");

            let mut template = LoginTemplate::new(base);

            // Check if the error is about email verification
            let error_message = match &e {
                Error::Validation(msg) if msg.contains("email address has not been verified") => {
                    msg.clone()
                }
                _ => "Invalid email or password".to_string(),
            };

            template.error = Some(error_message);
            template.redirect_to = form.redirect_to;

            let html = template.render().map_err(|e| {
                error!("Failed to render login template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

#[axum::debug_handler]
async fn logout(jar: CookieJar) -> Response {
    debug!("Processing logout");

    // Create a cookie that expires immediately to clear the auth
    let cookie = Cookie::build(("auth_token", ""))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
        .max_age(Default::default())
        .build();

    (jar.remove(cookie), response::redirect("/")).into_response()
}

// Email Verification Routes

async fn verify_email_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering email verification page");

    let mut base = BaseContext::new().with_page("verify-email");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = EmailVerificationTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render email verification template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
struct VerifyEmailForm {
    code: String,
    email: String,
}

#[axum::debug_handler]
async fn verify_email(
    jar: CookieJar,
    Form(form): Form<VerifyEmailForm>,
) -> Result<Response, Error> {
    debug!("Processing email verification for: {}", form.email);

    // Find the person by email
    let person = Person::find_by_email(&form.email)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    // Verify the code
    match VerificationService::verify_code(&person.id, &form.code, CodeType::EmailVerification)
        .await
    {
        Ok(_) => {
            // Mark email as verified
            VerificationService::mark_email_verified(&person.id)
                .await
                .map_err(|e| Error::Internal(format!("Failed to mark email as verified: {}", e)))?;

            info!("Email verified for user: {}", form.email);

            // Create authentication token for the verified user
            let token =
                crate::auth::create_jwt(&person.id.to_string(), &person.username, &person.email)?;

            // Create authentication cookie
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_default() == "true")
                .build();

            // Redirect to profile
            Ok((jar.add(cookie), response::redirect("/profile")).into_response())
        }
        Err(e) => {
            error!("Email verification failed for {}: {}", form.email, e);

            // Re-render the form with error
            let base = BaseContext::new().with_page("verify-email");

            let mut template = EmailVerificationTemplate::new(base);
            template.error = Some("Invalid or expired verification code".to_string());
            template.email = Some(form.email);

            let html = template.render().map_err(|e| {
                error!(
                    "Failed to render email verification template with error: {}",
                    e
                );
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

// Password Reset Routes

async fn forgot_password_form(request: Request) -> Result<Html<String>, Error> {
    debug!("Rendering forgot password page");

    let mut base = BaseContext::new().with_page("forgot-password");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let template = ForgotPasswordTemplate::new(base);

    let html = template.render().map_err(|e| {
        error!("Failed to render forgot password template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
struct ForgotPasswordForm {
    email: String,
}

#[axum::debug_handler]
async fn forgot_password(Form(form): Form<ForgotPasswordForm>) -> Result<Response, Error> {
    debug!("Processing password reset request for: {}", form.email);

    // Find the person by email
    if let Some(person) = Person::find_by_email(&form.email).await? {
        // Generate password reset code
        let reset_code =
            VerificationService::create_verification_code(&person.id, CodeType::PasswordReset)
                .await
                .map_err(|e| Error::Internal(format!("Failed to create reset code: {}", e)))?;

        // Send password reset email
        if let Ok(email_service) = EmailService::from_env() {
            let email_clone = form.email.clone();
            let person_name = person.name.clone();
            tokio::spawn(async move {
                if let Err(e) = email_service
                    .send_password_reset_email(&email_clone, person_name.as_deref(), &reset_code)
                    .await
                {
                    error!(
                        "Failed to send password reset email to {}: {}",
                        email_clone, e
                    );
                } else {
                    info!("Password reset email sent to {}", email_clone);
                }
            });
        }
    }

    // Always show success message to prevent email enumeration
    let base = BaseContext::new().with_page("forgot-password");

    let mut template = ForgotPasswordTemplate::new(base);
    template.success = Some(format!(
        "If an account exists for {}, a password reset code has been sent.",
        form.email
    ));

    let html = template.render().map_err(|e| {
        error!(
            "Failed to render forgot password template with success: {}",
            e
        );
        Error::template(e.to_string())
    })?;

    Ok(Html(html).into_response())
}

#[derive(Debug, Deserialize)]
struct ResetPasswordQuery {
    email: Option<String>,
    code: Option<String>,
}

#[axum::debug_handler]
async fn reset_password_form(
    Query(query): Query<ResetPasswordQuery>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Rendering reset password page");

    let mut base = BaseContext::new().with_page("reset-password");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let mut template = ResetPasswordTemplate::new(base);
    template.email = query.email;
    template.code = query.code;

    let html = template.render().map_err(|e| {
        error!("Failed to render reset password template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[derive(Debug, Deserialize)]
struct ResetPasswordForm {
    email: String,
    code: String,
    password: String,
    password_confirm: String,
}

#[axum::debug_handler]
async fn reset_password(Form(form): Form<ResetPasswordForm>) -> Result<Response, Error> {
    debug!("Processing password reset for: {}", form.email);

    // Validate passwords match
    if form.password != form.password_confirm {
        let base = BaseContext::new().with_page("reset-password");

        let mut template = ResetPasswordTemplate::new(base);
        template.error = Some("Passwords do not match".to_string());
        template.email = Some(form.email);
        template.code = Some(form.code);

        let html = template.render().map_err(|e| {
            error!("Failed to render reset password template with error: {}", e);
            Error::template(e.to_string())
        })?;

        return Ok(Html(html).into_response());
    }

    // Find the person by email
    let person = Person::find_by_email(&form.email)
        .await?
        .ok_or_else(|| Error::NotFound)?;

    // Verify the reset code
    match VerificationService::verify_code(&person.id, &form.code, CodeType::PasswordReset).await {
        Ok(_) => {
            // Update the password
            use crate::auth;
            use crate::db::DB;

            let password_hash = auth::hash_password(&form.password)?;

            let sql = "UPDATE person SET password = $password WHERE id = $id";
            DB.query(sql)
                .bind(("password", password_hash))
                .bind(("id", person.id.clone()))
                .await
                .map_err(|e| Error::Database(e.to_string()))?;

            info!("Password reset successful for user: {}", form.email);

            // Redirect to login page
            Ok(response::redirect("/login").into_response())
        }
        Err(e) => {
            error!(
                "Password reset verification failed for {}: {}",
                form.email, e
            );

            let base = BaseContext::new().with_page("reset-password");

            let mut template = ResetPasswordTemplate::new(base);
            template.error = Some("Invalid or expired reset code".to_string());
            template.email = Some(form.email);
            template.code = Some(form.code);

            let html = template.render().map_err(|e| {
                error!("Failed to render reset password template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

// Resend Verification Email Route

#[derive(Debug, Deserialize)]
struct ResendVerificationForm {
    email: String,
}

#[axum::debug_handler]
async fn resend_verification(Form(form): Form<ResendVerificationForm>) -> Result<Response, Error> {
    debug!("Processing resend verification request for: {}", form.email);

    // Find the person by email
    if let Some(person) = Person::find_by_email(&form.email).await? {
        if person.verification_status != "unverified" {
            debug!("User {} already verified, skipping resend", form.email);
        } else {
            // Generate new verification code
            let verification_code = VerificationService::create_verification_code(
                &person.id,
                CodeType::EmailVerification,
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to create verification code: {}", e)))?;

            // Send verification email
            if let Ok(email_service) = EmailService::from_env() {
                let email_clone = form.email.clone();
                let person_name = person.name.clone();
                tokio::spawn(async move {
                    if let Err(e) = email_service
                        .send_verification_email(
                            &email_clone,
                            person_name.as_deref(),
                            &verification_code,
                        )
                        .await
                    {
                        error!(
                            "Failed to resend verification email to {}: {}",
                            email_clone, e
                        );
                    } else {
                        info!("Verification email resent to {}", email_clone);
                    }
                });
            }
        }
    }

    // Always redirect to verify-email page to prevent email enumeration
    Ok(response::redirect("/verify-email").into_response())
}
