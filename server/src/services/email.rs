use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;
use tracing::{debug, error, info};

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("Failed to send email: {0}")]
    SendError(String),
    #[error("Missing configuration: {0}")]
    ConfigError(String),
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("JSON serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, EmailError>;

#[derive(Debug, Clone)]
pub struct EmailService {
    api_key: String,
    api_secret: String,
    from_email: String,
    from_name: String,
    base_url: String,
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

impl EmailService {
    /// Create a new EmailService instance from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key = env::var("MAILJET_API_KEY")
            .map_err(|_| EmailError::ConfigError("MAILJET_API_KEY not set".to_string()))?;
        let api_secret = env::var("MAILJET_API_SECRET")
            .map_err(|_| EmailError::ConfigError("MAILJET_API_SECRET not set".to_string()))?;
        let from_email =
            env::var("MAILJET_FROM_EMAIL").unwrap_or_else(|_| "noreply@slatehub.com".to_string());
        let from_name = env::var("MAILJET_FROM_NAME").unwrap_or_else(|_| "SlateHub".to_string());

        let client = reqwest::Client::new();

        Ok(EmailService {
            api_key,
            api_secret,
            from_email,
            from_name,
            base_url: "https://api.mailjet.com/v3.1".to_string(),
            client,
        })
    }

    /// Send an email through Mailjet
    async fn send_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        subject: &str,
        text_body: Option<&str>,
        html_body: Option<&str>,
    ) -> Result<()> {
        let message = Message {
            from: EmailAddress {
                email: self.from_email.clone(),
                name: Some(self.from_name.clone()),
            },
            to: vec![EmailAddress {
                email: to_email.to_string(),
                name: to_name.map(|n| n.to_string()),
            }],
            subject: subject.to_string(),
            text_part: text_body.map(|t| t.to_string()),
            html_part: html_body.map(|h| h.to_string()),
        };

        let payload = MailjetMessage {
            messages: vec![message],
        };

        debug!("Sending email to {} with subject: {}", to_email, subject);

        let response = self
            .client
            .post(format!("{}/send", self.base_url))
            .basic_auth(&self.api_key, Some(&self.api_secret))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            info!("Email sent successfully to {}", to_email);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(
                "Failed to send email. Status: {}, Error: {}",
                status, error_text
            );
            Err(EmailError::SendError(format!(
                "Mailjet API error: {} - {}",
                status, error_text
            )))
        }
    }

    /// Send email verification code
    pub async fn send_verification_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        verification_code: &str,
    ) -> Result<()> {
        let subject = "Verify your SlateHub email address";

        let text_body = format!(
            "Welcome to SlateHub!\n\n\
            Your verification code is: {}\n\n\
            Please enter this code on the verification page to complete your registration.\n\n\
            This code will expire in 24 hours.\n\n\
            If you didn't create an account on SlateHub, please ignore this email.\n\n\
            Best regards,\n\
            The SlateHub Team",
            verification_code
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
        <p style="font-size: 16px; margin-bottom: 20px;">Your verification code is:</p>

        <div style="background-color: #f0f4f8; border: 2px dashed #4a90e2; border-radius: 6px; padding: 20px; text-align: center; margin: 20px 0;">
            <code style="font-size: 32px; font-weight: bold; color: #4a90e2; letter-spacing: 4px;">{}</code>
        </div>

        <p style="font-size: 14px; color: #666; margin-top: 20px;">
            Please enter this code on the verification page to complete your registration.
        </p>

        <p style="font-size: 14px; color: #999; margin-top: 20px;">
            This code will expire in 24 hours. If you didn't create an account on SlateHub, please ignore this email.
        </p>
    </div>

    <div style="margin-top: 30px; padding-top: 20px; border-top: 1px solid #e0e0e0; text-align: center; color: #999; font-size: 12px;">
        <p>© 2024 SlateHub. All rights reserved.</p>
    </div>
</body>
</html>"#,
            verification_code
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

    /// Send password reset email
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        reset_code: &str,
    ) -> Result<()> {
        let subject = "Reset your SlateHub password";

        let text_body = format!(
            "Hello {},\n\n\
            We received a request to reset your SlateHub password.\n\n\
            Your password reset code is: {}\n\n\
            To reset your password:\n\
            1. Go to: https://slatehub.com/reset-password?email={}\n\
            2. Enter the code above\n\
            3. Create your new password\n\n\
            This code will expire in 1 hour.\n\n\
            If you didn't request a password reset, please ignore this email. Your password will remain unchanged.\n\n\
            Best regards,\n\
            The SlateHub Team",
            to_name.unwrap_or("there"),
            reset_code,
            urlencoding::encode(to_email)
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
            <a href="https://slatehub.com/reset-password?email={}" style="display: inline-block; background-color: #dc3545; color: white; padding: 12px 30px; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">Reset Your Password</a>
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
            reset_code,
            urlencoding::encode(to_email)
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
}

/// Initialize the global email service
pub async fn init() -> Result<EmailService> {
    EmailService::from_env()
}
