//! Transactional email via Postmark or Mailjet, chosen at runtime from the
//! environment.
//!
//! Covers every outbound mail the app sends: email-verification codes,
//! password resets, org/production invitations, generic notifications
//! (e.g. new-message alerts), and user feedback forwarding. Bodies are
//! built inline as paired plain-text + HTML strings; user-supplied content
//! interpolated into HTML is sanitized with `ammonia` first.
//!
//! There is no global instance or boot-time init: call sites construct an
//! [`EmailService`] with [`EmailService::from_env`] right before sending
//! (and usually skip sending, with a log line, when it fails).
//!
//! ## Provider selection
//!
//! [`EmailService::from_env`] picks a backend once, at construction
//! ([`select_provider_kind`] holds the precedence):
//! * `EMAIL_PROVIDER` — optional explicit override, `postmark` or `mailjet`.
//! * Otherwise auto-detect, preferring Postmark when its token is set and
//!   falling back to Mailjet. A deployment can keep both configured and
//!   switch by flipping which credentials (or `EMAIL_PROVIDER`) are present.
//!
//! ## Env vars
//!
//! Postmark:
//! * `POSTMARK_SERVER_TOKEN` — server token (sent as `X-Postmark-Server-Token`).
//! * `POSTMARK_MESSAGE_STREAM` — optional, default `outbound`.
//!
//! Mailjet:
//! * `MAILJET_API_KEY` / `MAILJET_API_SECRET` — basic-auth credentials.
//!
//! Shared sender identity (the `EMAIL_FROM_*` names are preferred; the
//! `MAILJET_FROM_*` names are still honored for backward compatibility):
//! * `EMAIL_FROM_ADDRESS` / `MAILJET_FROM_EMAIL` — default `noreply@slatehub.com`.
//! * `EMAIL_FROM_NAME` / `MAILJET_FROM_NAME` — default `SlateHub`.
//! * `FEEDBACK_RECIPIENT_EMAIL` — optional, where
//!   [`EmailService::send_feedback_email`] delivers (defaults to the from
//!   address).

use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use tracing::{debug, error, info};

/// Errors produced by [`EmailService`].
#[derive(Error, Debug)]
pub enum EmailError {
    /// The provider accepted the connection but answered non-2xx; carries the
    /// status and response body.
    #[error("Failed to send email: {0}")]
    SendError(String),
    /// No provider could be resolved from the environment, or the selected
    /// provider's credentials are missing.
    #[error("Missing configuration: {0}")]
    ConfigError(String),
    /// Transport-level failure talking to the provider's API.
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    /// Payload serialization failure (effectively unreachable for our
    /// static payload shapes).
    #[error("JSON serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, EmailError>;

/// The transactional-email backend to talk to, with its credentials. Chosen
/// once by [`EmailService::from_env`] and not changed afterward.
#[derive(Debug, Clone)]
enum Provider {
    /// Mailjet REST API (`/v3.1/send`), HTTP basic auth.
    Mailjet { api_key: String, api_secret: String },
    /// Postmark REST API (`/email`), `X-Postmark-Server-Token` header. The
    /// `message_stream` selects which stream the message goes out on.
    Postmark {
        server_token: String,
        message_stream: String,
    },
}

impl Provider {
    /// Short lowercase identifier, used in logs and error messages.
    fn name(&self) -> &'static str {
        match self {
            Provider::Mailjet { .. } => "mailjet",
            Provider::Postmark { .. } => "postmark",
        }
    }

    /// Build the Postmark variant, reading the optional message-stream
    /// override (`POSTMARK_MESSAGE_STREAM`, default `outbound`).
    fn postmark(server_token: String) -> Self {
        let message_stream = env::var("POSTMARK_MESSAGE_STREAM")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "outbound".to_string());
        Provider::Postmark {
            server_token,
            message_stream,
        }
    }
}

/// Which provider [`EmailService::from_env`] resolved to. Exposed so the
/// selection precedence can be unit-tested and surfaced (e.g. in logs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Postmark (`POSTMARK_SERVER_TOKEN`).
    Postmark,
    /// Mailjet (`MAILJET_API_KEY` + `MAILJET_API_SECRET`).
    Mailjet,
}

/// A configured email backend plus the sender identity. Cheap to build per
/// call; holds no connection state beyond a `reqwest::Client`.
#[derive(Debug, Clone)]
pub struct EmailService {
    provider: Provider,
    from_email: String,
    from_name: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct MailjetMessage {
    #[serde(rename = "Messages")]
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "From")]
    from: EmailAddress,
    #[serde(rename = "To")]
    to: Vec<EmailAddress>,
    #[serde(rename = "Subject")]
    subject: String,
    #[serde(rename = "TextPart", skip_serializing_if = "Option::is_none")]
    text_part: Option<String>,
    #[serde(rename = "HTMLPart", skip_serializing_if = "Option::is_none")]
    html_part: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EmailAddress {
    #[serde(rename = "Email")]
    email: String,
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

// --- Postmark wire format (`/email`) ---

/// Postmark single-send payload. Addresses are `Name <email>` strings; only
/// the fields we use are modeled.
#[derive(Debug, Serialize)]
struct PostmarkMessage {
    #[serde(rename = "From")]
    from: String,
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "Subject")]
    subject: String,
    #[serde(rename = "TextBody", skip_serializing_if = "Option::is_none")]
    text_body: Option<String>,
    #[serde(rename = "HtmlBody", skip_serializing_if = "Option::is_none")]
    html_body: Option<String>,
    #[serde(rename = "MessageStream")]
    message_stream: String,
}

/// A fully-rendered message handed to a provider transport. Bundled so the
/// per-provider senders stay within sane argument counts.
struct OutgoingEmail<'a> {
    to_email: &'a str,
    to_name: Option<&'a str>,
    subject: &'a str,
    text_body: Option<&'a str>,
    html_body: Option<&'a str>,
}

/// Render an address the way Postmark expects: `Name <email>` when a non-empty
/// display name is present, otherwise the bare address.
pub fn format_address(email: &str, name: Option<&str>) -> String {
    match name {
        Some(n) if !n.trim().is_empty() => format!("{n} <{email}>"),
        _ => email.to_string(),
    }
}

/// Decide which provider to use from already-read environment inputs. Pure
/// (no env access) so the precedence rules are unit-testable:
///
/// * An explicit value (`EMAIL_PROVIDER`) wins, but errors when that
///   provider's credentials are absent or the value is unrecognized.
/// * Otherwise Postmark is preferred when available, then Mailjet.
///
/// # Errors
///
/// [`EmailError::ConfigError`] when nothing is configured, the explicitly
/// requested provider lacks credentials, or `explicit` is an unknown name.
pub fn select_provider_kind(
    explicit: Option<&str>,
    has_postmark: bool,
    has_mailjet: bool,
) -> Result<ProviderKind> {
    match explicit.map(str::trim).map(str::to_lowercase).as_deref() {
        Some("postmark") => has_postmark
            .then_some(ProviderKind::Postmark)
            .ok_or_else(|| {
                EmailError::ConfigError(
                    "EMAIL_PROVIDER=postmark but POSTMARK_SERVER_TOKEN is not set".to_string(),
                )
            }),
        Some("mailjet") => has_mailjet.then_some(ProviderKind::Mailjet).ok_or_else(|| {
            EmailError::ConfigError(
                "EMAIL_PROVIDER=mailjet but MAILJET_API_KEY/MAILJET_API_SECRET are not set"
                    .to_string(),
            )
        }),
        Some(other) if !other.is_empty() => Err(EmailError::ConfigError(format!(
            "Unknown EMAIL_PROVIDER '{other}' (expected 'postmark' or 'mailjet')"
        ))),
        // No (or empty) explicit choice: auto-detect, Postmark first.
        _ if has_postmark => Ok(ProviderKind::Postmark),
        _ if has_mailjet => Ok(ProviderKind::Mailjet),
        _ => Err(EmailError::ConfigError(
            "No email provider configured: set POSTMARK_SERVER_TOKEN or \
             MAILJET_API_KEY/MAILJET_API_SECRET"
                .to_string(),
        )),
    }
}

impl EmailService {
    /// Build an [`EmailService`] from the environment: select the provider
    /// (see the module docs) and read the shared sender identity.
    ///
    /// # Errors
    ///
    /// [`EmailError::ConfigError`] when no provider can be resolved, when an
    /// explicit `EMAIL_PROVIDER` names one whose credentials are absent, or
    /// when `EMAIL_PROVIDER` holds an unknown value. The from-address vars
    /// always have defaults.
    pub fn from_env() -> Result<Self> {
        let provider = Self::provider_from_env()?;
        let from_email = env::var("EMAIL_FROM_ADDRESS")
            .or_else(|_| env::var("MAILJET_FROM_EMAIL"))
            .unwrap_or_else(|_| "noreply@slatehub.com".to_string());
        let from_name = env::var("EMAIL_FROM_NAME")
            .or_else(|_| env::var("MAILJET_FROM_NAME"))
            .unwrap_or_else(|_| "SlateHub".to_string());

        debug!("Email provider selected: {}", provider.name());

        Ok(EmailService {
            provider,
            from_email,
            from_name,
            client: reqwest::Client::new(),
        })
    }

    /// Read the provider credentials from the environment and resolve the
    /// active [`Provider`] using [`select_provider_kind`]'s precedence.
    fn provider_from_env() -> Result<Provider> {
        fn non_empty(var: &str) -> Option<String> {
            env::var(var)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }

        let postmark_token = non_empty("POSTMARK_SERVER_TOKEN");
        let mailjet_key = non_empty("MAILJET_API_KEY");
        let mailjet_secret = non_empty("MAILJET_API_SECRET");
        let explicit = non_empty("EMAIL_PROVIDER");

        match select_provider_kind(
            explicit.as_deref(),
            postmark_token.is_some(),
            mailjet_key.is_some() && mailjet_secret.is_some(),
        )? {
            ProviderKind::Postmark => Ok(Provider::postmark(
                postmark_token.expect("token present when ProviderKind::Postmark is selected"),
            )),
            ProviderKind::Mailjet => Ok(Provider::Mailjet {
                api_key: mailjet_key.expect("key present when ProviderKind::Mailjet is selected"),
                api_secret: mailjet_secret
                    .expect("secret present when ProviderKind::Mailjet is selected"),
            }),
        }
    }

    /// Send a single email through the configured provider.
    ///
    /// # Errors
    ///
    /// [`EmailError::HttpError`] on transport failure, [`EmailError::SendError`]
    /// when the provider answers non-2xx (bad credentials, rejected recipient,
    /// quota). All the public `send_*` wrappers share these failure modes.
    async fn send_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        subject: &str,
        text_body: Option<&str>,
        html_body: Option<&str>,
    ) -> Result<()> {
        let email = OutgoingEmail {
            to_email,
            to_name,
            subject,
            text_body,
            html_body,
        };

        debug!(
            "Sending email to {} via {} with subject: {}",
            to_email,
            self.provider.name(),
            subject
        );

        match &self.provider {
            Provider::Mailjet {
                api_key,
                api_secret,
            } => self.send_via_mailjet(api_key, api_secret, &email).await,
            Provider::Postmark {
                server_token,
                message_stream,
            } => {
                self.send_via_postmark(server_token, message_stream, &email)
                    .await
            }
        }
    }

    /// POST a message to Mailjet's `/v3.1/send` endpoint.
    async fn send_via_mailjet(
        &self,
        api_key: &str,
        api_secret: &str,
        email: &OutgoingEmail<'_>,
    ) -> Result<()> {
        let payload = MailjetMessage {
            messages: vec![Message {
                from: EmailAddress {
                    email: self.from_email.clone(),
                    name: Some(self.from_name.clone()),
                },
                to: vec![EmailAddress {
                    email: email.to_email.to_string(),
                    name: email.to_name.map(|n| n.to_string()),
                }],
                subject: email.subject.to_string(),
                text_part: email.text_body.map(|t| t.to_string()),
                html_part: email.html_body.map(|h| h.to_string()),
            }],
        };

        let response = self
            .client
            .post("https://api.mailjet.com/v3.1/send")
            .basic_auth(api_key, Some(api_secret))
            .json(&payload)
            .send()
            .await?;

        Self::handle_response(response, email.to_email, "Mailjet").await
    }

    /// POST a message to Postmark's `/email` endpoint.
    async fn send_via_postmark(
        &self,
        server_token: &str,
        message_stream: &str,
        email: &OutgoingEmail<'_>,
    ) -> Result<()> {
        let payload = PostmarkMessage {
            from: format_address(&self.from_email, Some(&self.from_name)),
            to: format_address(email.to_email, email.to_name),
            subject: email.subject.to_string(),
            text_body: email.text_body.map(|t| t.to_string()),
            html_body: email.html_body.map(|h| h.to_string()),
            message_stream: message_stream.to_string(),
        };

        let response = self
            .client
            .post("https://api.postmarkapp.com/email")
            .header("Accept", "application/json")
            .header("X-Postmark-Server-Token", server_token)
            .json(&payload)
            .send()
            .await?;

        Self::handle_response(response, email.to_email, "Postmark").await
    }

    /// Shared success/error handling for a provider's HTTP response.
    async fn handle_response(
        response: reqwest::Response,
        to_email: &str,
        provider: &str,
    ) -> Result<()> {
        if response.status().is_success() {
            info!("Email sent successfully to {} via {}", to_email, provider);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(
                "Failed to send email via {}. Status: {}, Error: {}",
                provider, status, error_text
            );
            Err(EmailError::SendError(format!(
                "{provider} API error: {status} - {error_text}"
            )))
        }
    }

    /// Send the email-verification message: a confirm link
    /// (`/verify-email/confirm?code=…&email=…` on [`crate::config::app_url`])
    /// plus the bare 6-digit code for manual entry. Tells the user the code
    /// expires in 24 hours (the TTL set by `services::verification`).
    pub async fn send_verification_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        verification_code: &str,
    ) -> Result<()> {
        let subject = "Verify your SlateHub email address";
        let base_url = crate::config::app_url();
        let verify_url = format!(
            "{}/verify-email/confirm?code={}&email={}",
            base_url,
            urlencoding::encode(verification_code),
            urlencoding::encode(to_email)
        );

        let text_body = format!(
            "Welcome to SlateHub!\n\n\
            Click the link below to verify your email:\n\
            {}\n\n\
            Or enter this code on the verification page:\n\
            {}\n\n\
            This code will expire in 24 hours.\n\n\
            If you didn't create an account on SlateHub, please ignore this email.\n\n\
            Best regards,\n\
            The SlateHub Team",
            verify_url, verification_code
        );

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #f8f9fa; border-radius: 8px; padding: 30px; margin-bottom: 20px;">
        <h1 style="color: #2c3e50; margin-top: 0;">Welcome to SlateHub!</h1>
        <p style="font-size: 16px; color: #555;">Thank you for joining our creative community.</p>
    </div>

    <div style="background-color: #ffffff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 30px;">
        <div style="text-align: center; margin: 20px 0 30px 0;">
            <a href="{}" style="display: inline-block; background-color: #eb5437; color: white; padding: 14px 36px; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">Verify My Email</a>
        </div>

        <div style="border-top: 1px solid #e0e0e0; padding-top: 20px; margin-top: 10px;">
            <p style="font-size: 14px; color: #666; margin-bottom: 10px;">Or enter this code on the verification page:</p>

            <div style="background-color: #f0f4f8; border: 2px dashed #4a90e2; border-radius: 6px; padding: 20px; text-align: center; margin: 10px 0;">
                <code style="font-size: 32px; font-weight: bold; color: #4a90e2; letter-spacing: 4px;">{}</code>
            </div>
        </div>

        <p style="font-size: 14px; color: #999; margin-top: 20px;">
            This code will expire in 24 hours. If you didn't create an account on SlateHub, please ignore this email.
        </p>
    </div>

    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>&copy; 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
            verify_url, verification_code
        );

        self.send_email(
            to_email,
            to_name,
            subject,
            Some(&text_body),
            Some(&html_body),
        )
        .await
    }

    /// Send the password-reset message: the 6-digit reset code plus a link
    /// to `/reset-password?email=…`. Tells the user the code expires in
    /// 1 hour (the TTL set by `services::verification`).
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        reset_code: &str,
    ) -> Result<()> {
        let subject = "Reset your SlateHub password";
        let base_url = crate::config::app_url();
        let encoded_email = urlencoding::encode(to_email);

        let text_body = format!(
            "Hello {},\n\n\
            We received a request to reset your SlateHub password.\n\n\
            Your password reset code is: {}\n\n\
            To reset your password:\n\
            1. Go to: {}/reset-password?email={}\n\
            2. Enter the code above\n\
            3. Create your new password\n\n\
            This code will expire in 1 hour.\n\n\
            If you didn't request a password reset, please ignore this email. Your password will remain unchanged.\n\n\
            Best regards,\n\
            The SlateHub Team",
            to_name.unwrap_or("there"),
            reset_code,
            base_url,
            encoded_email
        );

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #fff3cd; border: 1px solid #ffc107; border-radius: 8px; padding: 20px; margin-bottom: 20px;">
        <h2 style="color: #856404; margin-top: 0;">Password Reset Request</h2>
        <p style="color: #856404; margin-bottom: 0;">We received a request to reset your SlateHub password.</p>
    </div>

    <div style="background-color: #ffffff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 30px;">
        <p style="font-size: 16px; margin-bottom: 20px;">Your password reset code is:</p>

        <div style="background-color: #f0f4f8; border: 2px dashed #dc3545; border-radius: 6px; padding: 20px; text-align: center; margin: 20px 0;">
            <code style="font-size: 32px; font-weight: bold; color: #dc3545; letter-spacing: 4px;">{}</code>
        </div>

        <div style="text-align: center; margin: 30px 0;">
            <a href="{}/reset-password?email={}" style="display: inline-block; background-color: #dc3545; color: white; padding: 12px 30px; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">Reset Your Password</a>
        </div>

        <p style="font-size: 14px; color: #666; margin-top: 20px;">
            Click the button above or enter the code on the password reset page to create a new password.
        </p>

        <p style="font-size: 14px; color: #dc3545; font-weight: bold; margin-top: 20px;">
            This code will expire in 1 hour.
        </p>

        <p style="font-size: 14px; color: #999; margin-top: 20px;">
            If you didn't request a password reset, please ignore this email. Your password will remain unchanged.
        </p>
    </div>

    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>© 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
            reset_code, base_url, encoded_email
        );

        self.send_email(
            to_email,
            to_name,
            subject,
            Some(&text_body),
            Some(&html_body),
        )
        .await
    }

    /// Send an invitation email to someone with no SlateHub account yet
    /// (used for both org and production invites — `org_name` is whichever
    /// the target is). The optional personal `message` is sanitized with
    /// `ammonia` before being embedded in the HTML body; `signup_url` should
    /// carry the `ref=invite&email=…` query so signup auto-joins them.
    pub async fn send_invitation_email(
        &self,
        to_email: &str,
        org_name: &str,
        inviter_name: &str,
        signup_url: &str,
        message: Option<&str>,
    ) -> Result<()> {
        let subject = format!("You've been invited to join {} on SlateHub", org_name);

        let message_text = match message {
            Some(msg) if !msg.is_empty() => format!("\n\n{} says: \"{}\"\n", inviter_name, msg),
            _ => String::new(),
        };

        let text_body = format!(
            "Hi there!\n\n\
            {} has invited you to join {} on SlateHub — the production networking platform.{}\n\n\
            To accept this invitation, create your free account:\n\
            {}\n\n\
            Once you sign up and verify your email, you'll automatically be added to {}.\n\n\
            If you weren't expecting this invitation, you can safely ignore this email.\n\n\
            Best regards,\n\
            The SlateHub Team",
            inviter_name, org_name, message_text, signup_url, org_name
        );

        let message_html = match message {
            Some(msg) if !msg.is_empty() => format!(
                r#"<div style="background-color: #f5f5f5; border-left: 3px solid #eb5437; padding: 15px 20px; margin: 20px 0; border-radius: 4px;">
            <p style="font-size: 14px; color: #666; margin: 0 0 5px 0; font-weight: 600;">{} says:</p>
            <p style="font-size: 15px; color: #333; margin: 0; font-style: italic;">"{}"</p>
        </div>"#,
                inviter_name,
                ammonia::clean(msg)
            ),
            _ => String::new(),
        };

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #171717; border-radius: 8px; padding: 30px; margin-bottom: 20px;">
        <h1 style="color: #d6d8ca; margin-top: 0;">You're Invited!</h1>
        <p style="font-size: 16px; color: #d6d8ca;">{} has invited you to join <strong>{}</strong> on SlateHub.</p>
    </div>

    <div style="background-color: #ffffff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 30px;">
        <p style="font-size: 16px; margin-bottom: 20px;">
            SlateHub is the production networking platform for film, TV, and media professionals.
        </p>

        {}

        <div style="text-align: center; margin: 30px 0;">
            <a href="{}" style="display: inline-block; background-color: #eb5437; color: white; padding: 14px 36px; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">Create Your Account</a>
        </div>

        <p style="font-size: 14px; color: #666; margin-top: 20px;">
            Once you sign up and verify your email, you'll automatically be added to {}.
        </p>

        <p style="font-size: 14px; color: #999; margin-top: 20px;">
            If you weren't expecting this invitation, you can safely ignore this email.
        </p>
    </div>

    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>&copy; 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
            inviter_name, org_name, message_html, signup_url, org_name
        );

        self.send_email(to_email, None, &subject, Some(&text_body), Some(&html_body))
            .await
    }

    /// Send a generic notification email (e.g., new message notification).
    /// The caller supplies ready-made text and HTML bodies; this just
    /// forwards them to Mailjet unchanged.
    pub async fn send_notification_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        subject: &str,
        text_body: &str,
        html_body: &str,
    ) -> Result<()> {
        self.send_email(to_email, to_name, subject, Some(text_body), Some(html_body))
            .await
    }

    /// Forward a user-submitted feedback message to the operators. Recipient
    /// is `FEEDBACK_RECIPIENT_EMAIL`, falling back to the configured from
    /// address; the message is `ammonia`-sanitized for the HTML body.
    pub async fn send_feedback_email(
        &self,
        username: &str,
        page_url: &str,
        message: &str,
    ) -> Result<()> {
        let recipient =
            env::var("FEEDBACK_RECIPIENT_EMAIL").unwrap_or_else(|_| self.from_email.clone());

        let subject = format!("SlateHub Feedback from {}", username);
        let clean_message = ammonia::clean(message);

        let text_body = format!(
            "New feedback from SlateHub\n\n\
            User: {}\n\
            Page: {}\n\n\
            Message:\n{}\n",
            username, page_url, message
        );

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <div style="background-color: #171717; border-radius: 8px; padding: 30px; margin-bottom: 20px;">
        <h1 style="color: #d6d8ca; margin-top: 0;">New Feedback</h1>
        <p style="font-size: 14px; color: #999; margin-bottom: 0;">From <strong style="color: #d6d8ca;">{}</strong> on <code style="color: #eb5437;">{}</code></p>
    </div>

    <div style="background-color: #ffffff; border: 1px solid #e0e0e0; border-radius: 8px; padding: 30px;">
        <p style="font-size: 16px; white-space: pre-wrap;">{}</p>
    </div>

    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>&copy; 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
            username, page_url, clean_message
        );

        self.send_email(
            &recipient,
            None,
            &subject,
            Some(&text_body),
            Some(&html_body),
        )
        .await
    }
}

/// Async-flavored convenience constructor; identical to
/// [`EmailService::from_env`]. Note there is no global email singleton —
/// call sites build the service from env per send, and nothing in the boot
/// path currently calls this function.
///
/// # Errors
///
/// Same as [`EmailService::from_env`]: [`EmailError::ConfigError`] when no
/// provider can be resolved from the environment.
pub async fn init() -> Result<EmailService> {
    EmailService::from_env()
}
