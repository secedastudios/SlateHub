//! Authentication routes: signup (honeypot + form-token timing +
//! proof-of-work spam layers, IP rate limiting), login/logout with the
//! `auth_token` JWT cookie, email verification (code form and direct
//! link), password reset, resend-verification, and `/i/{token}` short
//! invite links that either join the target directly or land on signup.

use askama::Template;
use axum::{
    Form, Router,
    extract::{ConnectInfo, Query, Request},
    http::HeaderMap,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::{LazyLock, Mutex, Once};
use std::time::Instant;

use spow::pow::Pow;
use tracing::{debug, error, info, warn};

/// Initialize spow once at first use
static SPOW_INIT: Once = Once::new();
fn ensure_spow_init() {
    SPOW_INIT.call_once(|| {
        if let Err(e) = Pow::init_random() {
            error!("Failed to initialize spow: {}", e);
        }
    });
}

/// Generate a PoW challenge (valid for 300 seconds / 5 minutes)
fn generate_pow_challenge() -> String {
    ensure_spow_init();
    Pow::with_difficulty(20, 300)
        .map(|p| p.build_challenge())
        .unwrap_or_default()
}

/// Generate a signed form token encoding the current timestamp.
/// Uses jsonwebtoken to create a short-lived token.
fn generate_form_token() -> String {
    use jsonwebtoken::{EncodingKey, Header, encode};
    #[derive(serde::Serialize)]
    struct FormClaims {
        iat: i64,
    }
    let claims = FormClaims {
        iat: chrono::Utc::now().timestamp(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(
            std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "fallback-secret".to_string())
                .as_bytes(),
        ),
    )
    .unwrap_or_default()
}

/// Validate the form token and check minimum elapsed time (3 seconds).
fn validate_form_token(token: &str) -> bool {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
    #[derive(serde::Deserialize)]
    struct FormClaims {
        iat: i64,
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.required_spec_claims = std::collections::HashSet::new();
    validation.validate_exp = false;
    validation.validate_aud = false;
    let data = decode::<FormClaims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "fallback-secret".to_string())
                .as_bytes(),
        ),
        &validation,
    );
    match data {
        Ok(token_data) => {
            let elapsed = chrono::Utc::now().timestamp() - token_data.claims.iat;
            debug!("Form token elapsed: {}s", elapsed);
            (3..=600).contains(&elapsed)
        }
        Err(e) => {
            debug!("Form token decode error: {}", e);
            false
        }
    }
}

/// Simple in-memory per-IP rate limiter for signup — a coarse backstop behind
/// the per-request honeypot / form-token / proof-of-work layers.
static SIGNUP_RATE_LIMIT: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Max signups per resolved client IP per hour. Configurable via
/// `SIGNUP_MAX_PER_HOUR` (default 20) so it can be raised in production without
/// a redeploy — important because ad traffic shares mobile-carrier (CGNAT) IPs
/// and in-app browsers funnel many real users through a few addresses. The
/// previous hard-coded `3` was blocking legitimate signups during campaigns.
static SIGNUP_MAX_PER_HOUR: LazyLock<usize> = LazyLock::new(|| {
    env::var("SIGNUP_MAX_PER_HOUR")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(20)
});
const SIGNUP_WINDOW_SECS: u64 = 3600;

fn check_signup_rate_limit(ip: &str) -> bool {
    let mut map = SIGNUP_RATE_LIMIT.lock().unwrap();
    let now = Instant::now();
    let attempts = map.entry(ip.to_string()).or_default();
    attempts.retain(|t| now.duration_since(*t).as_secs() < SIGNUP_WINDOW_SECS);
    if attempts.len() >= *SIGNUP_MAX_PER_HOUR {
        false
    } else {
        attempts.push(now);
        true
    }
}

/// Client-IP precedence, pure for testing: left-most `X-Forwarded-For` entry,
/// then `X-Real-IP`, then the socket peer address. The socket fallback means
/// an unidentified client is keyed by its real connection address rather than
/// collapsed with everyone else into one shared bucket (which a `"unknown"`
/// literal would do, blocking all signups once the bucket fills).
pub fn resolve_client_ip(
    forwarded_for: Option<&str>,
    real_ip: Option<&str>,
    peer: IpAddr,
) -> String {
    forwarded_for
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| real_ip.map(str::trim).filter(|s| !s.is_empty()))
        .map(|s| s.to_string())
        .unwrap_or_else(|| peer.to_string())
}

fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> String {
    resolve_client_ip(
        headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()),
        headers.get("x-real-ip").and_then(|v| v.to_str().ok()),
        peer.ip(),
    )
}

use crate::{
    error::Error,
    middleware::UserExtractor,
    models::person::{CreateUser, LoginUser, Person},
    record_id_ext::RecordIdExt,
    response,
    services::{
        email::EmailService,
        landing::{self, Event},
        verification::{CodeType, VerificationService},
    },
    templates::{
        BaseContext, EmailVerificationTemplate, ForgotPasswordTemplate, LoginTemplate,
        ResetPasswordTemplate, SignupTemplate, User, VerifyConversionTemplate,
    },
};

/// Routes for signup, login/logout, email verification, password reset,
/// and short invite links.
pub fn router() -> Router {
    Router::new()
        .route("/i/{token}", get(invite_link))
        .route("/signup", get(signup_form).post(signup))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
        .route("/verify-email", get(verify_email_form).post(verify_email))
        .route("/verify-email/confirm", get(verify_email_link))
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

#[derive(Debug, Deserialize)]
struct SignupQuery {
    email: Option<String>,
    redirect: Option<String>,
    /// Landing campaign id carried from `/a/{campaign}` (attribution).
    campaign: Option<String>,
    /// Selected role chip — analytics only, never applied to the account.
    role: Option<String>,
}

/// A [`SignupTemplate`] with freshly-minted anti-bot tokens (PoW challenge +
/// form token) and the pixel id already set. EVERY render of the signup form —
/// the GET page and the post-error re-render alike — must start from here.
/// Re-rendering with a stale or empty `form_token` / `pow_challenge` blocks the
/// user's resubmit with a 422, which is exactly the bug this prevents.
fn fresh_signup_template(base: BaseContext) -> SignupTemplate {
    let mut template = SignupTemplate::new(base);
    template.pow_challenge = generate_pow_challenge();
    template.form_token = generate_form_token();
    template.pixel_id = crate::config::meta_pixel_id();
    template
}

async fn signup_form(
    Query(query): Query<SignupQuery>,
    jar: CookieJar,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Rendering signup page");

    let mut base = BaseContext::new().with_page("signup");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    // Resolve the landing campaign: query param first, then the lp_campaign
    // cookie set on the landing page. Record `signup_started` so the funnel
    // captures campaign visitors who reached the form.
    let campaign = query
        .campaign
        .filter(|c| !c.is_empty())
        .or_else(|| jar.get("lp_campaign").map(|c| c.value().to_string()));
    if let Some(camp) = &campaign {
        landing::record_event(Event {
            campaign: camp.clone(),
            event_type: landing::event::SIGNUP_STARTED.to_string(),
            role: query.role.filter(|r| !r.is_empty()),
            visitor_id: jar.get("lp_vid").map(|c| c.value().to_string()),
            path: Some("/signup".to_string()),
            ..Default::default()
        });
    }

    let mut template = fresh_signup_template(base);
    template.prefill_email = query.email;
    template.redirect = query.redirect;
    template.campaign = campaign;

    let html = template.render().map_err(|e| {
        error!("Failed to render signup template: {}", e);
        Error::template(e.to_string())
    })?;

    Ok(Html(html))
}

#[axum::debug_handler]
async fn signup(
    headers: HeaderMap,
    jar: CookieJar,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Form(form): Form<CreateUser>,
) -> Result<Response, Error> {
    debug!("Processing signup for email: {}", form.email);

    // Resolved client IP + campaign, attached to every block/rejection log so
    // failures can be tallied by reason (rate_limit / honeypot / form_token /
    // pow) and by campaign.
    let ip = client_ip(&headers, peer);
    let campaign = form.campaign.as_deref().unwrap_or("-");

    // Coarse per-IP rate limit (configurable via SIGNUP_MAX_PER_HOUR).
    if !check_signup_rate_limit(&ip) {
        warn!(reason = "rate_limit", ip = %ip, campaign, "signup blocked");
        return Err(Error::Validation(
            "Too many signup attempts. Please try again later.".to_string(),
        ));
    }

    // Layer 1: Honeypot — reject if the hidden "website" field is filled
    if form.website.as_ref().is_some_and(|w| !w.is_empty()) {
        warn!(reason = "honeypot", ip = %ip, campaign, "signup blocked");
        return Err(Error::Validation(
            "Signup failed. Please try again.".to_string(),
        ));
    }

    // Layer 2: Time check — reject if form was submitted too fast (< 3 seconds)
    if let Some(ref token) = form.form_token {
        if !validate_form_token(token) {
            warn!(reason = "form_token", ip = %ip, campaign, "signup blocked: token invalid or too fast");
            return Err(Error::Validation(
                "Signup failed. Please try again.".to_string(),
            ));
        }
    } else {
        warn!(reason = "form_token_missing", ip = %ip, campaign, "signup blocked");
        return Err(Error::Validation(
            "Signup failed. Please try again.".to_string(),
        ));
    }

    // Layer 3: Proof-of-Work — reject if PoW solution is missing or invalid
    ensure_spow_init();
    match &form.pow_solution {
        Some(solution) if !solution.is_empty() => {
            if let Err(e) = Pow::validate(solution) {
                warn!(reason = "pow_invalid", ip = %ip, campaign, error = %e, "signup blocked");
                return Err(Error::Validation(
                    "Verification failed. Please reload and try again.".to_string(),
                ));
            }
        }
        _ => {
            warn!(reason = "pow_missing", ip = %ip, campaign, "signup blocked");
            return Err(Error::Validation(
                "Verification failed. Please reload and try again.".to_string(),
            ));
        }
    }

    // Try to create the user
    let email = form.email.clone();
    let redirect = form.redirect.clone();
    // Campaign attribution: hidden form field first, then the lp_campaign
    // cookie set on the landing page. Persisted on the person below so the
    // conversion (email verification) can be attributed in a later session.
    let campaign = form
        .campaign
        .clone()
        .filter(|c| !c.is_empty())
        .or_else(|| jar.get("lp_campaign").map(|c| c.value().to_string()));
    match Person::signup(form.username, form.email, form.password, Some(ip.clone())).await {
        Ok((token, person_id)) => {
            info!(ip = %ip, person_id = %person_id, "User created successfully");
            crate::services::activity::log_activity(
                Some(&person_id),
                "signup",
                &format!("/signup [ip:{}]", ip),
            );

            // Persist landing-page attribution (read at email verification to
            // record the conversion).
            if let Some(camp) = &campaign {
                landing::set_signup_campaign(&person_id, camp).await;
            }

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
                .build();

            // Redirect to email verification page, forwarding redirect param
            let mut verify_url = format!("/verify-email?email={}", urlencoding::encode(&email));
            if let Some(ref r) = redirect {
                verify_url.push_str(&format!("&redirect={}", urlencoding::encode(r)));
            }

            Ok((
                CookieJar::new().add(cookie),
                response::redirect(&verify_url),
            )
                .into_response())
        }
        Err(e) => {
            error!("Signup failed: {}", e);

            // Re-render with the error AND fresh anti-bot tokens — without them
            // the resubmit fails the form-token / PoW check (a 422). Keep the
            // entered email and redirect so the user doesn't retype everything.
            let base = BaseContext::new().with_page("signup");

            let mut template = fresh_signup_template(base);
            template.error = Some(e.to_string());
            template.prefill_email = Some(email);
            template.redirect = redirect;
            template.campaign = campaign;

            let html = template.render().map_err(|e| {
                error!("Failed to render signup template with error: {}", e);
                Error::template(e.to_string())
            })?;

            Ok(Html(html).into_response())
        }
    }
}

async fn login_form(
    Query(params): Query<std::collections::HashMap<String, String>>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Rendering login page");

    let mut base = BaseContext::new().with_page("login");

    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let mut template = LoginTemplate::new(base);
    template.redirect_to = params.get("redirect").cloned();

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
        Ok((token, person_id)) => {
            info!("User logged in successfully");
            crate::services::activity::log_activity(Some(&person_id), "login", "/login");

            // Create authentication cookie with the JWT token
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
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
        .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
        .max_age(Default::default())
        .build();

    (jar.remove(cookie), response::redirect("/")).into_response()
}

// Email Verification Routes

async fn verify_email_form(
    Query(params): Query<std::collections::HashMap<String, String>>,
    request: Request,
) -> Result<Html<String>, Error> {
    debug!("Rendering email verification page");

    let mut base = BaseContext::new().with_page("verify-email");

    // Add user to context if authenticated
    if let Some(user) = request.get_user() {
        base = base.with_user(User::from_session_user(&user).await);
    }

    let mut template = EmailVerificationTemplate::new(base);
    template.email = params.get("email").cloned();
    template.redirect = params.get("redirect").cloned();
    if params.contains_key("resent") {
        template.success =
            Some("A new verification code has been sent. Please check your email.".to_string());
    }
    template.pixel_id = crate::config::meta_pixel_id();

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
    #[serde(default)]
    redirect: Option<String>,
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
        .ok_or(Error::NotFound)?;

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

            // Welcome email from the founders. Fire-and-forget — a mail failure
            // must never block the freshly-verified user's redirect. verify_code
            // is single-use, so this fires once per account.
            if let Ok(email_service) = EmailService::from_env() {
                let to_email = person.email.clone();
                let to_name = person.name.clone();
                let app = crate::config::app_url();
                let invite_url = app.clone();
                let profile_url = format!("{}/{}", app, person.username);
                tokio::spawn(async move {
                    match email_service
                        .send_welcome_email(
                            &to_email,
                            to_name.as_deref(),
                            &invite_url,
                            &profile_url,
                        )
                        .await
                    {
                        Ok(()) => info!(email = %to_email, "welcome email sent"),
                        Err(e) => error!(email = %to_email, error = %e, "welcome email failed"),
                    }
                });
            } else {
                warn!("welcome email skipped: no email provider configured");
            }

            // Process any pending invitations for this email
            let person_id = person.id.to_raw_string();
            let redirect_url =
                match crate::services::invitation::InvitationService::process_pending_invitations(
                    &person_id,
                    &form.email,
                )
                .await
                {
                    Ok(Some(url)) => {
                        info!(
                            "Processed pending invitations for {}, redirecting to {}",
                            form.email, url
                        );
                        url
                    }
                    Ok(None) => form
                        .redirect
                        .clone()
                        .unwrap_or_else(|| "/profile/edit".to_string()),
                    Err(e) => {
                        error!(
                            "Failed to process pending invitations for {}: {}",
                            form.email, e
                        );
                        form.redirect
                            .clone()
                            .unwrap_or_else(|| "/profile/edit".to_string())
                    }
                };

            // Landing-page conversion: "email verified" IS the conversion. If
            // this person signed up through a campaign, record it (awaited, so
            // it's durable) and route the response through the pixel interstitial.
            let campaign = landing::get_signup_campaign(&person.id).await;
            if let Some(camp) = &campaign {
                landing::record_event_now(Event {
                    campaign: camp.clone(),
                    event_type: landing::event::SIGNUP_COMPLETED.to_string(),
                    person_id: Some(person_id.clone()),
                    path: Some("/verify-email".to_string()),
                    ..Default::default()
                })
                .await;
            }

            // Create authentication token + cookie for the verified user
            let token = crate::auth::create_jwt(&person_id, &person.username, &person.email)?;
            let cookie = Cookie::build(("auth_token", token))
                .path("/")
                .same_site(SameSite::Lax)
                .http_only(true)
                .secure(env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false")
                .build();

            // Campaign-attributed signups land on a brief interstitial that
            // fires the Meta Pixel CompleteRegistration before continuing;
            // everyone else gets a direct redirect (no UX change).
            if campaign.is_some() {
                let base = BaseContext::new().with_page("verify-success");
                let template = crate::with_base!(VerifyConversionTemplate, base, {
                    pixel_id: crate::config::meta_pixel_id(),
                    redirect: redirect_url,
                });
                let html = template.render().map_err(|e| {
                    error!("Failed to render verify-success template: {}", e);
                    Error::template(e.to_string())
                })?;
                Ok((jar.add(cookie), Html(html)).into_response())
            } else {
                Ok((jar.add(cookie), response::redirect(&redirect_url)).into_response())
            }
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

/// Direct email verification via link (GET with query params)
#[derive(Debug, Deserialize)]
struct VerifyEmailQuery {
    code: String,
    email: String,
}

async fn verify_email_link(
    jar: CookieJar,
    Query(query): Query<VerifyEmailQuery>,
) -> Result<Response, Error> {
    debug!(
        "Processing email verification via link for: {}",
        query.email
    );

    let form = VerifyEmailForm {
        code: query.code,
        email: query.email,
        redirect: None,
    };

    verify_email(jar, Form(form)).await
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
    template.email = Some(form.email.clone());

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
        .ok_or(Error::NotFound)?;

    // Verify the reset code
    match VerificationService::verify_code(&person.id, &form.code, CodeType::PasswordReset).await {
        Ok(_) => {
            // Update the password
            use crate::auth;
            use crate::db::DB;

            let password_hash = auth::hash_password(&form.password).await?;

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
    /// Post-verification redirect target, carried through from the verify
    /// form's hidden field so resending doesn't drop it.
    #[serde(default)]
    redirect: Option<String>,
}

#[axum::debug_handler]
async fn resend_verification(Form(form): Form<ResendVerificationForm>) -> Result<Response, Error> {
    info!(email = %form.email, "resend verification requested");

    // The user-facing response is intentionally identical in every branch
    // below (anti-enumeration); these logs are the only way to see what
    // actually happened, so they're at info/warn rather than debug.
    match Person::find_by_email(&form.email).await? {
        None => {
            info!(email = %form.email, "resend: no matching account — nothing sent");
        }
        Some(person) if person.verification_status != "unverified" => {
            info!(
                email = %form.email,
                status = %person.verification_status,
                "resend: account is not unverified — nothing sent"
            );
        }
        Some(person) => {
            // Generate a fresh verification code, then dispatch the email.
            let verification_code = VerificationService::create_verification_code(
                &person.id,
                CodeType::EmailVerification,
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to create verification code: {}", e)))?;

            match EmailService::from_env() {
                Ok(email_service) => {
                    let email_clone = form.email.clone();
                    let person_name = person.name.clone();
                    info!(email = %email_clone, "resend: dispatching verification email");
                    tokio::spawn(async move {
                        if let Err(e) = email_service
                            .send_verification_email(
                                &email_clone,
                                person_name.as_deref(),
                                &verification_code,
                            )
                            .await
                        {
                            error!(email = %email_clone, error = %e, "resend: send failed");
                        } else {
                            info!(email = %email_clone, "resend: verification email sent");
                        }
                    });
                }
                Err(e) => {
                    warn!(error = %e, "resend: no email provider configured — nothing sent");
                }
            }
        }
    }

    // Always redirect back to the verify page — carrying the email so the form
    // stays prefilled and a `resent` flag so the page shows a confirmation. The
    // response is identical whether or not the address existed, to avoid email
    // enumeration.
    let mut redirect_to = format!(
        "/verify-email?email={}&resent=1",
        urlencoding::encode(&form.email)
    );
    if let Some(r) = form.redirect.as_ref().filter(|r| !r.is_empty()) {
        redirect_to.push_str(&format!("&redirect={}", urlencoding::encode(r)));
    }
    Ok(response::redirect(&redirect_to).into_response())
}

/// Handle short invite links: /i/{token}
/// If logged in: process the invitation and redirect to the target
/// If not logged in: show landing page with OG tags that auto-redirects to signup
async fn invite_link(
    axum::extract::Path(token): axum::extract::Path<String>,
    request: Request,
) -> Result<Response, Error> {
    use crate::models::pending_invitation::PendingInvitationModel;
    use crate::templates::InviteLandingTemplate;

    warn!("INVITE_LINK token={}", token);
    let pi_model = PendingInvitationModel::new();
    let invite = match pi_model.find_by_token(&token).await? {
        Some(inv) => {
            warn!("INVITE_LINK found target={}", inv.target_slug);
            inv
        }
        None => {
            warn!("INVITE_LINK not_found token={}", token);
            return Err(Error::NotFound);
        }
    };

    let user_opt = request.get_user();

    if let Some(user) = user_opt {
        // Logged in — process the invitation directly
        let redirect_url = match invite.target_type.as_str() {
            "production" => {
                use crate::models::production::ProductionModel;
                let prod = ProductionModel::get_by_slug(&invite.target_slug).await?;
                ProductionModel::add_member_accepted(
                    &prod.id,
                    &user.id,
                    &invite.role,
                    invite.production_roles.clone(),
                )
                .await?;
                pi_model.mark_accepted(&invite.id.to_raw_string()).await?;
                format!("/productions/{}", invite.target_slug)
            }
            "organization" => {
                use crate::models::organization::OrganizationModel;
                let org_model = OrganizationModel::new();
                org_model
                    .add_member(&invite.target_id, &user.id, &invite.role, None)
                    .await?;
                pi_model.mark_accepted(&invite.id.to_raw_string()).await?;
                format!("/orgs/{}", invite.target_slug)
            }
            _ => "/".to_string(),
        };

        Ok(axum::response::Redirect::to(&redirect_url).into_response())
    } else {
        // Not logged in — show landing page with OG meta tags
        let invite_path = format!("/i/{}", token);
        let redirect_url = if let Some(email) = &invite.email {
            format!(
                "/signup?email={}&redirect={}",
                urlencoding::encode(email),
                urlencoding::encode(&invite_path),
            )
        } else {
            format!("/signup?redirect={}", urlencoding::encode(&invite_path))
        };

        // Fetch poster URL for production invites
        let poster_url = if invite.target_type == "production" {
            use crate::models::production::ProductionModel;
            ProductionModel::get_by_slug(&invite.target_slug)
                .await
                .ok()
                .and_then(|p| p.poster_photo.or(p.poster_url))
        } else {
            None
        };

        let base = crate::templates::BaseContext::new().with_page("invite");
        let template = crate::with_base!(InviteLandingTemplate, base, {
            target_name: invite.target_name.clone(),
            target_type: invite.target_type.clone(),
            production_roles: invite.production_roles.clone(),
            poster_url,
            redirect_url,
            token: token.clone(),
        });

        let html = template.render().map_err(|e| {
            error!("Failed to render invite landing template: {}", e);
            Error::template(e.to_string())
        })?;

        Ok(axum::response::Html(html).into_response())
    }
}
