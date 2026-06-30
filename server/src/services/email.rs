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
use std::collections::HashMap;
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
    #[serde(rename = "Cc", skip_serializing_if = "Option::is_none")]
    cc: Option<Vec<EmailAddress>>,
    // Mailjet's structured `ReplyTo` is a single address; a custom header
    // carries a comma-separated multi-address Reply-To instead.
    #[serde(rename = "Headers", skip_serializing_if = "Option::is_none")]
    headers: Option<HashMap<String, String>>,
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
    #[serde(rename = "Cc", skip_serializing_if = "Option::is_none")]
    cc: Option<String>,
    #[serde(rename = "ReplyTo", skip_serializing_if = "Option::is_none")]
    reply_to: Option<String>,
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
    /// Override the configured sender identity (used by the founder welcome
    /// email so it comes "from Chris & Tom"). `None` uses the service default.
    from_email: Option<&'a str>,
    from_name: Option<&'a str>,
    /// Optional single CC recipient.
    cc: Option<&'a str>,
    /// Optional `Reply-To` (comma-separated for multiple addresses) so replies
    /// reach addresses other than the `From` — e.g. both founders.
    reply_to: Option<&'a str>,
}

/// Render an address the way Postmark expects: `Name <email>` when a non-empty
/// display name is present, otherwise the bare address.
pub fn format_address(email: &str, name: Option<&str>) -> String {
    match name {
        Some(n) if !n.trim().is_empty() => format!("{n} <{email}>"),
        _ => email.to_string(),
    }
}

/// Minimal HTML text escape for interpolating user-supplied values (a person's
/// name) into an HTML email body.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build a profile-completion reminder email: `(subject, text, html)`.
///
/// `reminder_number` (1, 2, or 3) drives the tone — playful, then pointed, then
/// the serious final notice. `edit_url` links to the profile editor (it bounces
/// through login if the recipient is logged out); `grace_days` is how long after
/// the final reminder the account is removed (only reminder 3 mentions it).
/// Pure, so the copy is unit-testable.
pub fn profile_reminder_bodies(
    first_name: Option<&str>,
    reminder_number: u8,
    edit_url: &str,
    grace_days: u32,
) -> (String, String, String) {
    let first = first_name
        .and_then(|n| n.split_whitespace().next())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let text_greeting = match first {
        Some(n) => format!("Hey {n},"),
        None => "Hey,".to_string(),
    };
    let html_greeting = match first {
        Some(n) => format!("Hey {},", escape_html(n)),
        None => "Hey,".to_string(),
    };

    // (subject, plain message, HTML message paragraphs, CTA label) per reminder.
    let (subject, message_text, message_html, cta_label): (String, String, String, &str) =
        match reminder_number {
            1 => (
                "Your SlateHub profile is gloriously empty".to_string(),
                "You signed up, verified your email, and then left your profile completely blank. No photo, no credits, nothing for anyone to find. Go on, give us something to work with.".to_string(),
                "<p style=\"margin:0 0 22px;\">You signed up, verified your email, and then left your profile completely blank. No photo, no credits, nothing for anyone to find. Go on, give us something to work with.</p>".to_string(),
                "Finish my profile",
            ),
            2 => (
                "Your SlateHub profile is still blank".to_string(),
                "Second nudge. An empty profile means you don't show up when a producer or casting director searches SlateHub for someone like you. That's the whole reason to be here.".to_string(),
                "<p style=\"margin:0 0 22px;\">Second nudge. An empty profile means you don't show up when a producer or casting director searches SlateHub for someone like you. That's the whole reason to be here.</p>".to_string(),
                "Finish my profile",
            ),
            _ => (
                "Final notice: we'll remove your SlateHub account soon".to_string(),
                format!(
                    "Last reminder. SlateHub only works if the directory is real, so we don't keep empty profiles on it, and yours is still blank.\n\nIf we don't hear back, we'll remove the account in {grace_days} days. You're welcome back any time."
                ),
                format!(
                    "<p style=\"margin:0 0 18px;\">Last reminder. SlateHub only works if the directory is real, so we don't keep empty profiles on it, and yours is still blank.</p><p style=\"margin:0 0 22px;\">If we don't hear back, we'll remove the account in {grace_days} days. You're welcome back any time.</p>"
                ),
                "Keep my spot",
            ),
        };

    let text_body = format!(
        "{text_greeting}\n\n{message_text}\n\n{cta_label}: {edit_url}\n\nChris & Tom\nSlateHub"
    );

    let html_body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><meta name="color-scheme" content="light"></head>
<body style="margin:0; padding:0; background-color:#171717;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#171717;">
        <tr><td align="center" style="padding:28px 16px;">
            <table role="presentation" width="600" cellpadding="0" cellspacing="0" style="width:100%; max-width:600px;">
                <tr><td style="padding:30px 38px 22px; background-color:#171717;">
                    <div style="font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:22px; font-weight:700; letter-spacing:0.10em; text-transform:uppercase; color:#d6d8ca;">SlateHub</div>
                </td></tr>
                <tr><td style="padding:34px 38px 30px; background-color:#ffffff; font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:16px; line-height:1.65; color:#2a2a2a;">
                    <p style="margin:0 0 18px;">{html_greeting}</p>
                    {message_html}
                    <table role="presentation" cellpadding="0" cellspacing="0" style="margin:0 0 24px;"><tr><td style="border-radius:6px; background-color:#eb5437;">
                        <a href="{edit_url}" style="display:inline-block; padding:13px 30px; font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:15px; font-weight:700; letter-spacing:0.03em; color:#ffffff; text-decoration:none;">{cta_label}</a>
                    </td></tr></table>
                    <p style="margin:0; color:#6b6b6b; font-size:14px;">Chris &amp; Tom, SlateHub</p>
                </td></tr>
            </table>
        </td></tr>
    </table>
</body>
</html>"#
    );

    (subject, text_body, html_body)
}

/// A founder's mini-card in the welcome email, built from their live profile at
/// send time so the photo, name, and title stay current. `avatar_url` and
/// `profile_url` must be absolute — email clients can't resolve relative paths.
pub struct FounderTag {
    pub name: String,
    pub title: String,
    pub avatar_url: String,
    pub profile_url: String,
}

/// The founders' story video, rendered as a clickable poster — email clients
/// don't play inline video, so it links out to watch.
pub struct WelcomeVideo<'a> {
    pub thumbnail_url: &'a str,
    pub watch_url: &'a str,
}

/// Everything the welcome email needs beyond the recipient. Bundled so the pure
/// body builder stays a single, testable call.
pub struct WelcomeEmail<'a> {
    /// Recipient's name if known (the greeting uses the first token).
    pub recipient_name: Option<&'a str>,
    /// Link recipients share to invite others (the site root).
    pub invite_url: &'a str,
    /// The recipient's own public profile page.
    pub profile_url: &'a str,
    /// Founder mini-cards (Chris & Tom), from their live profiles.
    pub founders: &'a [FounderTag],
    /// Optional founders' story video.
    pub video: Option<WelcomeVideo<'a>>,
    /// Instagram follow link and the handle to display.
    pub instagram_url: &'a str,
    pub instagram_handle: &'a str,
}

/// Build the founder welcome email: `(subject, text_body, html_body)`.
///
/// Sent once, the moment a new member verifies their email. The voice is Chris
/// & Tom's, the message is "you joined a free, open community built by
/// filmmakers," and the single call to action is to invite other filmmakers.
/// The founder cards, story video, and links are passed in (resolved at send
/// time from the live profiles) so this stays pure and unit-testable.
pub fn welcome_email_bodies(ctx: &WelcomeEmail<'_>) -> (String, String, String) {
    let first = ctx
        .recipient_name
        .and_then(|n| n.split_whitespace().next())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let subject = match first {
        Some(n) => format!("Welcome to SlateHub, {n}"),
        None => "Welcome to SlateHub".to_string(),
    };

    let text_greeting = match first {
        Some(n) => format!("Hey {n},"),
        None => "Hey,".to_string(),
    };

    // Dynamic trailing block for the text version: video, sign-off, founder
    // directory, Instagram.
    let mut text_tail = String::new();
    if let Some(v) = &ctx.video {
        text_tail.push_str(&format!(
            "If you want the two-minute version of why we built this, here it is.\n\
             Watch our story (2 min): {}\n\n",
            v.watch_url
        ));
    }
    text_tail.push_str("Thanks for being one of the early ones.\n\nChris and Tom\nSlateHub\n\n");
    for f in ctx.founders {
        text_tail.push_str(&format!("{}, {}: {}\n", f.name, f.title, f.profile_url));
    }
    text_tail.push_str(&format!(
        "Follow along on Instagram: {} ({})",
        ctx.instagram_handle, ctx.instagram_url
    ));

    let text_body = format!(
        "{greeting}\n\n\
         Chris here, and Tom's on this email too. You just verified your email, so you're officially part of SlateHub. Welcome.\n\n\
         We built this because we're filmmakers, and we got tired of watching good crew and cast get stuck behind paywalls and pay-to-play directories. So we made the opposite. SlateHub is free to join and free to use, and it always will be. No subscription. No fee to be seen.\n\n\
         What we're really trying to build is the largest free directory of filmmakers and creators anywhere: crew, cast, editors, composers, and creators of every kind, in one place anyone can search. The whole platform is open source, so you can look under the hood and see exactly what we do with your work and your data. We don't sell it.\n\n\
         Your profile is your home base. It works like a Linktree made for film: reels, credits, links, and contact on one page you can share anywhere. Yours lives at {profile_url}. We also build every profile to be found, optimized for search so producers, casting directors, and collaborators can turn up your work on Google, not just inside an app.\n\n\
         A quick note, because the word 'verified' comes up twice on SlateHub. What you just did is email verification: it confirms a real person with a real inbox, and it's free. Separate from that, and completely optional, is identity verification. That's a one-time payment that just covers our costs. It unlocks a few extra features and helps keep out the bots and spam that wreck most free sites. Your profile works fully without it.\n\n\
         We're always adding features and improvements, and the best ideas come from people actually using SlateHub. If there's something important you need us to build, tell us. Just reply to this email, it reaches us directly.\n\n\
         Now the ask. SlateHub grows the way film always has, through the people in it. If you know others who make things, invite them here: {invite_url}. Every filmmaker who joins makes this more useful for the rest of us.\n\n\
         {tail}",
        greeting = text_greeting,
        profile_url = ctx.profile_url,
        invite_url = ctx.invite_url,
        tail = text_tail,
    );

    let html_greeting = match first {
        Some(n) => format!("Hey {},", escape_html(n)),
        None => "Hey,".to_string(),
    };

    // Story video as a clickable poster (no inline playback in email).
    let video_html = match &ctx.video {
        Some(v) => format!(
            r#"<p style="margin:0 0 12px;">If you want the two-minute version of why we built this, here it is.</p>
                            <table role="presentation" cellpadding="0" cellspacing="0" style="margin:0 0 26px;"><tr><td>
                                <a href="{watch}" style="text-decoration:none;"><img src="{thumb}" width="524" alt="Watch the SlateHub founders story" style="display:block; width:100%; max-width:524px; height:auto; border-radius:8px; border:0;"><span style="display:inline-block; margin-top:10px; color:#eb5437; font-weight:700; font-size:14px;">Watch our story (2 min)</span></a>
                            </td></tr></table>"#,
            watch = v.watch_url,
            thumb = v.thumbnail_url,
        ),
        None => String::new(),
    };

    // Founder mini-cards (avatar + name + title, linked to their profiles).
    let founders_html: String = ctx
        .founders
        .iter()
        .map(|f| {
            format!(
                r#"<tr>
                                <td style="padding:8px 12px 8px 0; vertical-align:middle;"><a href="{profile}"><img src="{avatar}" width="46" height="46" alt="{name}" style="display:block; width:46px; height:46px; border-radius:50%; object-fit:cover; border:0;"></a></td>
                                <td style="vertical-align:middle; font-family:'Helvetica Neue',Helvetica,Arial,sans-serif;"><a href="{profile}" style="color:#2a2a2a; text-decoration:none; font-weight:600; font-size:15px;">{name}</a><br><span style="color:#6b6b6b; font-size:13px;">{title}</span></td>
                            </tr>"#,
                profile = f.profile_url,
                avatar = f.avatar_url,
                name = escape_html(&f.name),
                title = escape_html(&f.title),
            )
        })
        .collect();

    let html_body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="color-scheme" content="light">
</head>
<body style="margin:0; padding:0; background-color:#171717;">
    <div style="display:none; max-height:0; overflow:hidden; opacity:0;">You're in. A note from Chris and Tom about the free, open community we're building for filmmakers.</div>
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#171717;">
        <tr>
            <td align="center" style="padding:28px 16px;">
                <table role="presentation" width="600" cellpadding="0" cellspacing="0" style="width:100%; max-width:600px;">
                    <tr>
                        <td style="padding:34px 38px 26px; background-color:#171717;">
                            <div style="font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:24px; font-weight:700; letter-spacing:0.10em; text-transform:uppercase; color:#d6d8ca;">SlateHub</div>
                            <div style="font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:13px; letter-spacing:0.02em; color:#9ca39e; margin-top:7px;">By filmmakers, for filmmakers.</div>
                        </td>
                    </tr>
                    <tr>
                        <td style="padding:36px 38px 12px; background-color:#ffffff; font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:16px; line-height:1.65; color:#2a2a2a;">
                            <p style="margin:0 0 18px;">{greeting}</p>
                            <p style="margin:0 0 18px;">Chris here, and Tom's on this email too. You just verified your email, so you're officially part of SlateHub. Welcome.</p>
                            <p style="margin:0 0 18px;">We built this because we're filmmakers, and we got tired of watching good crew and cast get stuck behind paywalls and pay-to-play directories. So we made the opposite. SlateHub is free to join and free to use, and it always will be. No subscription. No fee to be seen.</p>
                            <p style="margin:0 0 18px;">What we're really trying to build is the largest free directory of filmmakers and creators anywhere: crew, cast, editors, composers, and creators of every kind, in one place anyone can search. The whole platform is open source, so you can look under the hood and see exactly what we do with your work and your data. We don't sell it.</p>
                            <p style="margin:0 0 18px;">Your profile is your home base. It works like a Linktree made for film: reels, credits, links, and contact on one page you can share anywhere. Yours lives at <a href="{profile_url}" style="color:#eb5437; text-decoration:none; font-weight:600;">{profile_url}</a>. We also build every profile to be found, optimized for search so producers, casting directors, and collaborators can turn up your work on Google, not just inside an app.</p>
                            <p style="margin:0 0 18px;">A quick note, because the word 'verified' comes up twice on SlateHub. What you just did is email verification: it confirms a real person with a real inbox, and it's free. Separate from that, and completely optional, is identity verification. That's a one-time payment that just covers our costs. It unlocks a few extra features and helps keep out the bots and spam that wreck most free sites. Your profile works fully without it.</p>
                            <p style="margin:0 0 18px;">We're always adding features and improvements, and the best ideas come from people actually using SlateHub. If there's something important you need us to build, tell us. Just reply to this email, it reaches us directly.</p>
                            <p style="margin:0 0 22px;">Now the ask. SlateHub grows the way film always has, through the people in it. If you know others who make things, invite them to <a href="{invite_url}" style="color:#eb5437; text-decoration:none; font-weight:600;">SlateHub</a>. Every filmmaker who joins makes this more useful for the rest of us.</p>
                            {video_html}
                            <p style="margin:0 0 4px;">Thanks for being one of the early ones.</p>
                            <p style="margin:18px 0 12px;"><span style="font-weight:600; color:#2a2a2a;">Chris and Tom</span><br><span style="color:#6b6b6b; font-size:14px;">SlateHub</span></p>
                            <table role="presentation" cellpadding="0" cellspacing="0" style="margin:0;">
                                {founders_html}
                            </table>
                        </td>
                    </tr>
                    <tr>
                        <td style="padding:18px 38px 30px; background-color:#ffffff; border-top:1px solid #ece9e2; font-family:'Helvetica Neue',Helvetica,Arial,sans-serif; font-size:12px; line-height:1.6; color:#9a9a9a;">
                            <p style="margin:0 0 5px;">Follow along on Instagram: <a href="{instagram_url}" style="color:#eb5437; text-decoration:none; font-weight:600;">{instagram_handle}</a></p>
                            <p style="margin:0 0 5px;">You're getting this because you just joined SlateHub at <a href="{invite_url}" style="color:#eb5437; text-decoration:none;">slatehub.com</a>.</p>
                            <p style="margin:0;">Free forever. Open source. Built by filmmakers.</p>
                        </td>
                    </tr>
                </table>
            </td>
        </tr>
    </table>
</body>
</html>"#,
        greeting = html_greeting,
        profile_url = ctx.profile_url,
        invite_url = ctx.invite_url,
        video_html = video_html,
        founders_html = founders_html,
        instagram_url = ctx.instagram_url,
        instagram_handle = ctx.instagram_handle,
    );

    (subject, text_body, html_body)
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
        self.dispatch(OutgoingEmail {
            to_email,
            to_name,
            subject,
            text_body,
            html_body,
            from_email: None,
            from_name: None,
            cc: None,
            reply_to: None,
        })
        .await
    }

    /// Send a fully-built message through the configured provider.
    async fn dispatch(&self, email: OutgoingEmail<'_>) -> Result<()> {
        debug!(
            "Sending email to {} via {} with subject: {}",
            email.to_email,
            self.provider.name(),
            email.subject
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
                    email: email.from_email.unwrap_or(&self.from_email).to_string(),
                    name: Some(email.from_name.unwrap_or(&self.from_name).to_string()),
                },
                to: vec![EmailAddress {
                    email: email.to_email.to_string(),
                    name: email.to_name.map(|n| n.to_string()),
                }],
                cc: email.cc.map(|c| {
                    vec![EmailAddress {
                        email: c.to_string(),
                        name: None,
                    }]
                }),
                headers: email
                    .reply_to
                    .map(|r| HashMap::from([("Reply-To".to_string(), r.to_string())])),
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
            from: format_address(
                email.from_email.unwrap_or(&self.from_email),
                Some(email.from_name.unwrap_or(&self.from_name)),
            ),
            to: format_address(email.to_email, email.to_name),
            cc: email.cc.map(|c| c.to_string()),
            reply_to: email.reply_to.map(|r| r.to_string()),
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

    /// Send the founder welcome email, once, right after a member verifies
    /// their email. Comes "from Chris & Tom": the sender identity is overridden
    /// to `WELCOME_FROM_EMAIL` / `WELCOME_FROM_NAME` (defaults
    /// `chris@slatehub.com` / `Chris & Tom @SLATEHUB`), CC's
    /// `WELCOME_CC_EMAIL` (default `tom@slatehub.com`), and sets a `Reply-To`
    /// of `WELCOME_REPLY_TO` (default both founders) so replies reach Chris and
    /// Tom even on a plain "Reply". Copy is built by [`welcome_email_bodies`].
    ///
    /// `invite_url` is the link recipients share to invite others; `profile_url`
    /// is the member's own public page.
    ///
    /// # Errors
    ///
    /// Same failure modes as the other senders (see [`Self::send_email`]).
    pub async fn send_welcome_email(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        invite_url: &str,
        profile_url: &str,
    ) -> Result<()> {
        // Resolve the founder cards from their live profiles (so the photo /
        // name / title are always current) and absolutize their URLs for email.
        let base = invite_url.trim_end_matches('/');
        let founders: Vec<FounderTag> = crate::services::landing::founders()
            .await
            .iter()
            .map(|f| {
                let avatar_url = if f.avatar.starts_with("http") {
                    f.avatar.clone()
                } else {
                    format!("{}{}", base, f.avatar)
                };
                FounderTag {
                    name: f.name.clone(),
                    title: f.title.clone(),
                    avatar_url,
                    profile_url: format!("{}/{}", base, f.username),
                }
            })
            .collect();

        // The founders' story video from the not-on-set landing page, as a
        // YouTube thumbnail that links out (email can't play video inline).
        let video_urls = crate::services::landing::find_campaign("not-on-set").map(|c| {
            (
                format!("https://img.youtube.com/vi/{}/hqdefault.jpg", c.video_id),
                format!("https://www.youtube.com/watch?v={}", c.video_id),
            )
        });
        let video = video_urls
            .as_ref()
            .map(|(thumbnail_url, watch_url)| WelcomeVideo {
                thumbnail_url,
                watch_url,
            });

        let (subject, text_body, html_body) = welcome_email_bodies(&WelcomeEmail {
            recipient_name: to_name,
            invite_url,
            profile_url,
            founders: &founders,
            video,
            instagram_url: "https://www.instagram.com/slatehubofficial",
            instagram_handle: "@slatehubofficial",
        });

        let from_email = env::var("WELCOME_FROM_EMAIL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "chris@slatehub.com".to_string());
        let from_name = env::var("WELCOME_FROM_NAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Chris & Tom @SLATEHUB".to_string());
        let cc = env::var("WELCOME_CC_EMAIL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some("tom@slatehub.com".to_string()));
        // Reply-To both founders so any reply (even a plain "Reply", not just
        // "Reply All") reaches Chris and Tom, not just the From address.
        let reply_to = env::var("WELCOME_REPLY_TO")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "chris@slatehub.com, tom@slatehub.com".to_string());

        self.dispatch(OutgoingEmail {
            to_email,
            to_name,
            subject: &subject,
            text_body: Some(&text_body),
            html_body: Some(&html_body),
            from_email: Some(&from_email),
            from_name: Some(&from_name),
            cc: cc.as_deref(),
            reply_to: Some(&reply_to),
        })
        .await
    }

    /// Send a profile-completion reminder (1, 2, or 3) from the default sender.
    /// `edit_url` should point at the profile editor; `grace_days` is the
    /// removal window mentioned in the final reminder. Copy is built by
    /// [`profile_reminder_bodies`].
    ///
    /// # Errors
    ///
    /// Same failure modes as the other senders (see [`Self::send_email`]).
    pub async fn send_profile_reminder(
        &self,
        to_email: &str,
        to_name: Option<&str>,
        reminder_number: u8,
        edit_url: &str,
        grace_days: u32,
    ) -> Result<()> {
        let (subject, text_body, html_body) =
            profile_reminder_bodies(to_name, reminder_number, edit_url, grace_days);
        self.send_email(
            to_email,
            to_name,
            &subject,
            Some(&text_body),
            Some(&html_body),
        )
        .await
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
