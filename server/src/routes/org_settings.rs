//! Organization settings page (owner/admin only) including the API &
//! Integrations section that manages the org's OIDC client.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::error;

use crate::{
    config,
    error::Error,
    middleware::AuthenticatedUser,
    models::{
        oauth_client::{OauthClient, OauthClientModel},
        organization::{Organization, OrganizationModel},
    },
    record_id_ext::RecordIdExt,
    services::oidc_tokens::{self, SessionSummary},
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/orgs/{slug}/settings", get(settings_page))
        .route("/orgs/{slug}/settings/oidc/enable", post(enable_oidc))
        .route(
            "/orgs/{slug}/settings/oidc/rotate-secret",
            post(rotate_secret),
        )
        .route("/orgs/{slug}/settings/oidc/disable", post(disable_oidc))
        .route(
            "/orgs/{slug}/settings/oidc/redirect-uris",
            post(update_redirect_uris),
        )
        .route(
            "/orgs/{slug}/settings/oidc/post-logout-uris",
            post(update_post_logout_uris),
        )
        .route(
            "/orgs/{slug}/settings/oidc/scopes",
            post(update_allowed_scopes),
        )
        .route("/orgs/{slug}/settings/oidc/ssf", post(update_ssf))
        .route(
            "/orgs/{slug}/settings/oidc/sessions/revoke-all",
            post(revoke_all_sessions),
        )
        .route(
            "/orgs/{slug}/settings/oidc/sessions/{session_id}/revoke",
            post(revoke_session),
        )
}

#[derive(Template)]
#[template(path = "organizations/settings.html")]
pub struct OrganizationSettingsTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub organization: Organization,
    pub oidc: Option<OidcView>,
    pub new_secret: Option<String>,
    pub issuer: String,
    pub sessions: Vec<SessionSummary>,
    pub scope_checkboxes: Vec<ScopeCheckbox>,
    pub ssf_checkboxes: Vec<EventCheckbox>,
}

pub struct OidcView {
    pub client_id: String,
    pub redirect_uris_text: String,
    pub post_logout_uris_text: String,
    pub allowed_scopes: Vec<String>,
    pub ssf_endpoint: Option<String>,
    pub ssf_delivery: String,
    pub ssf_events: Vec<String>,
}

impl OidcView {
    fn from_client(c: &OauthClient) -> Self {
        Self {
            client_id: c.client_id.clone(),
            redirect_uris_text: c.redirect_uris.join("\n"),
            post_logout_uris_text: c.post_logout_redirect_uris.join("\n"),
            allowed_scopes: c.allowed_scopes.clone(),
            ssf_endpoint: c.ssf_receiver_endpoint.clone(),
            ssf_delivery: c.ssf_delivery_method.clone(),
            ssf_events: c.ssf_events_subscribed.clone(),
        }
    }
}

pub struct ScopeCheckbox {
    pub scope: String,
    pub checked: bool,
}

pub struct EventCheckbox {
    pub event_uri: String,
    pub label: String,
    pub checked: bool,
}

const ALL_SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "slatehub:org_membership",
];

async fn require_admin(slug: &str, user_id: &str) -> Result<Organization, Error> {
    let model = OrganizationModel::new();
    let organization = model.get_by_slug(slug).await?;
    let role = model
        .get_member_role(&organization.id.to_raw_string(), user_id)
        .await?;
    let is_admin = matches!(role.as_deref(), Some("owner") | Some("admin"));
    if !is_admin {
        return Err(Error::Forbidden);
    }
    Ok(organization)
}

async fn settings_page(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    Query(qs): Query<HashMap<String, String>>,
) -> Result<Html<String>, Error> {
    let organization = require_admin(&slug, &user.id).await?;

    let client_model = OauthClientModel::new();
    let client = client_model.get_for_organization(&organization.id).await?;
    let oidc = client.as_ref().map(OidcView::from_client);
    let sessions = match &client {
        Some(c) => oidc_tokens::list_sessions_for_client(&c.id)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let mut base = BaseContext::new().with_page("organization-settings");
    base = base.with_user(User::from_session_user(&user).await);

    let scope_checkboxes: Vec<ScopeCheckbox> = ALL_SCOPES
        .iter()
        .map(|s| ScopeCheckbox {
            scope: (*s).to_string(),
            checked: oidc
                .as_ref()
                .map(|o| o.allowed_scopes.iter().any(|x| x == s))
                .unwrap_or(false),
        })
        .collect();
    let ssf_event_defs: &[(&str, &str)] = &[
        (
            "https://schemas.openid.net/secevent/caep/event-type/token-claims-change",
            "CAEP token-claims-change",
        ),
        (
            "https://schemas.openid.net/secevent/caep/event-type/session-revoked",
            "CAEP session-revoked",
        ),
        (
            "https://schemas.openid.net/secevent/risc/event-type/account-disabled",
            "RISC account-disabled",
        ),
        (
            "https://schemas.slatehub.com/secevent/event-type/org-membership-revoked",
            "slatehub org-membership-revoked",
        ),
    ];
    let ssf_checkboxes: Vec<EventCheckbox> = ssf_event_defs
        .iter()
        .map(|(uri, label)| EventCheckbox {
            event_uri: (*uri).to_string(),
            label: (*label).to_string(),
            checked: oidc
                .as_ref()
                .map(|o| o.ssf_events.iter().any(|x| x == uri))
                .unwrap_or(false),
        })
        .collect();

    let template = OrganizationSettingsTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        organization,
        oidc,
        new_secret: qs.get("new_secret").cloned(),
        issuer: config::app_url(),
        sessions,
        scope_checkboxes,
        ssf_checkboxes,
    };

    Ok(Html(template.render().map_err(|e| {
        error!("Failed to render organization settings template: {}", e);
        Error::template(e.to_string())
    })?))
}

async fn enable_oidc(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let creds = OauthClientModel::new()
        .create(&organization.id, &organization.name)
        .await?;
    let target = format!(
        "/orgs/{}/settings?new_secret={}#api",
        slug,
        urlencoding::encode(&creds.plaintext_secret)
    );
    Ok(Redirect::to(&target).into_response())
}

async fn rotate_secret(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    let plaintext = model.rotate_secret(&client.id, 24).await?;
    let target = format!(
        "/orgs/{}/settings?new_secret={}#api",
        slug,
        urlencoding::encode(&plaintext)
    );
    Ok(Redirect::to(&target).into_response())
}

async fn disable_oidc(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    oidc_tokens::revoke_all_sessions_for_client(&client.id).await?;
    model.delete(&client.id).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct UriListForm {
    pub uris: String,
}

fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

async fn update_redirect_uris(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    Form(form): Form<UriListForm>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    let uris = parse_uri_list(&form.uris);
    for u in &uris {
        url::Url::parse(u).map_err(|e| Error::BadRequest(format!("invalid URI '{u}': {e}")))?;
    }
    model.update_redirect_uris(&client.id, uris).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

async fn update_post_logout_uris(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    Form(form): Form<UriListForm>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    let uris = parse_uri_list(&form.uris);
    for u in &uris {
        url::Url::parse(u).map_err(|e| Error::BadRequest(format!("invalid URI '{u}': {e}")))?;
    }
    model.update_post_logout_uris(&client.id, uris).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

async fn update_allowed_scopes(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    body: String,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_str(&body).map_err(|e| Error::BadRequest(e.to_string()))?;
    let scopes: Vec<String> = pairs
        .into_iter()
        .filter(|(k, _)| k == "scopes")
        .map(|(_, v)| v)
        .filter(|s| ALL_SCOPES.contains(&s.as_str()))
        .collect();
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    model.update_allowed_scopes(&client.id, scopes).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

async fn update_ssf(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
    body: String,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let pairs: Vec<(String, String)> =
        serde_urlencoded::from_str(&body).map_err(|e| Error::BadRequest(e.to_string()))?;
    let mut endpoint = None;
    let mut delivery = "push".to_string();
    let mut events = Vec::new();
    for (k, v) in pairs {
        match k.as_str() {
            "endpoint" if !v.trim().is_empty() => {
                endpoint = Some(v.trim().to_string());
            }
            "delivery_method" => delivery = v,
            "events" => events.push(v),
            _ => {}
        }
    }
    if let Some(ref ep) = endpoint {
        url::Url::parse(ep).map_err(|e| Error::BadRequest(format!("invalid endpoint: {e}")))?;
    }
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    model
        .update_ssf(&client.id, endpoint, delivery, events)
        .await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

async fn revoke_all_sessions(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<Response, Error> {
    let organization = require_admin(&slug, &user.id).await?;
    let model = OauthClientModel::new();
    let client = model
        .get_for_organization(&organization.id)
        .await?
        .ok_or(Error::NotFound)?;
    oidc_tokens::revoke_all_sessions_for_client(&client.id).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}

async fn revoke_session(
    AuthenticatedUser(user): AuthenticatedUser,
    Path((slug, session_id)): Path<(String, String)>,
) -> Result<Response, Error> {
    let _organization = require_admin(&slug, &user.id).await?;
    oidc_tokens::revoke_session(&session_id).await?;
    Ok(Redirect::to(&format!("/orgs/{}/settings#api", slug)).into_response())
}
