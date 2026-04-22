//! OpenID Connect provider endpoints.
//!
//! Routes mounted here:
//!   - GET  /.well-known/openid-configuration
//!   - GET  /.well-known/jwks.json
//!   - GET  /authorize
//!   - POST /authorize/consent
//!   - POST /token
//!   - GET  /userinfo  (also POST per spec)
//!   - POST /revoke
//!   - POST /introspect
//!   - GET  /logout    (RP-initiated)
//!
//! All security-event delivery code lives in `services::oidc_events`.

use askama::Template;
use axum::{
    Router,
    extract::{Form, Query, Request},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use surrealdb::types::{RecordId, SurrealValue};
use tracing::warn;
use url::Url;

use crate::{
    config,
    error::Error,
    middleware::UserExtractor,
    models::{
        consent_grant,
        oauth_client::{OauthClient, OauthClientModel},
        organization::OrganizationModel,
        person::Person,
    },
    record_id_ext::RecordIdExt,
    services::{
        oidc_keys,
        oidc_tokens::{
            self, ACCESS_TOKEN_TTL_SECONDS, CreateAccessToken, CreateAuthorizationCode,
            CreateRefreshToken,
        },
    },
    templates::{BaseContext, User},
};

mod filters {
    pub fn abs_url(path: &str) -> askama::Result<String> {
        Ok(format!("{}{}", crate::config::app_url(), path))
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/.well-known/jwks.json", get(jwks))
        .route("/authorize", get(authorize))
        .route("/authorize/consent", post(consent))
        .route("/token", post(token))
        .route("/userinfo", get(userinfo).post(userinfo))
        .route("/revoke", post(revoke))
        .route("/introspect", post(introspect))
        .route("/logout", get(rp_initiated_logout))
}

// ---------- Discovery + JWKS ----------

async fn discovery() -> Result<Json<Value>, Error> {
    let issuer = config::app_url();
    Ok(Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "revocation_endpoint": format!("{issuer}/revoke"),
        "introspection_endpoint": format!("{issuer}/introspect"),
        "end_session_endpoint": format!("{issuer}/logout"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "scopes_supported": ["openid", "profile", "email", "offline_access", "slatehub:org_membership"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "code_challenge_methods_supported": ["S256"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "subject_types_supported": ["public"],
        "claims_supported": [
            "sub", "iss", "aud", "exp", "iat", "auth_time", "nonce",
            "name", "preferred_username", "email", "email_verified",
            "slatehub_org", "slatehub_org_role", "slatehub_permissions"
        ]
    })))
}

async fn jwks() -> Result<Json<Value>, Error> {
    Ok(Json(oidc_keys::jwks_document().await?))
}

// ---------- /authorize ----------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthorizeParams {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: Option<String>,
}

#[derive(Template)]
#[template(path = "oidc/consent.html")]
pub struct ConsentTemplate {
    pub app_name: String,
    pub year: i32,
    pub version: String,
    pub active_page: String,
    pub user: Option<User>,
    pub client_name: String,
    pub org_name: String,
    pub requested_scopes: Vec<ScopeView>,
    pub already_granted: Vec<String>,
    pub params_json: String,
}

pub struct ScopeView {
    pub scope: String,
    pub description: String,
}

fn describe_scope(scope: &str, org_name: &str) -> String {
    match scope {
        "openid" => "Sign you in to this app".to_string(),
        "profile" => "Read your name and username".to_string(),
        "email" => "Read your email address".to_string(),
        "offline_access" => "Stay signed in by issuing refresh tokens".to_string(),
        "slatehub:org_membership" => format!("Read your role and permissions in {org_name}"),
        other => format!("Custom scope: {other}"),
    }
}

async fn authorize(
    Query(params): Query<AuthorizeParams>,
    request: Request,
) -> Result<Response, Error> {
    if params.response_type != "code" {
        return Err(Error::BadRequest(
            "unsupported response_type (only 'code')".into(),
        ));
    }
    let challenge_method = params
        .code_challenge_method
        .clone()
        .unwrap_or_else(|| "plain".into());
    if challenge_method != "S256" {
        return Err(Error::BadRequest(
            "code_challenge_method must be S256".into(),
        ));
    }

    let model = OauthClientModel::new();
    let client = model
        .get_by_client_id(&params.client_id)
        .await?
        .ok_or_else(|| Error::BadRequest("unknown client_id".into()))?;

    if !client
        .redirect_uris
        .iter()
        .any(|u| u == &params.redirect_uri)
    {
        return Err(Error::BadRequest("redirect_uri not registered".into()));
    }

    let requested_scopes: Vec<String> = params
        .scope
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if !requested_scopes.iter().any(|s| s == "openid") {
        return Err(Error::BadRequest("scope must include 'openid'".into()));
    }
    for s in &requested_scopes {
        if !client.allowed_scopes.iter().any(|a| a == s) {
            return Err(Error::BadRequest(format!("scope '{s}' not allowed")));
        }
    }

    // Bounce unauthenticated users to the login page, preserving the original URL.
    let user = match request.get_user() {
        Some(u) => u,
        None => {
            let original = build_authorize_url(&params);
            let target = format!("/login?redirect={}", urlencoding::encode(&original));
            return Ok(Redirect::to(&target).into_response());
        }
    };

    let person_id =
        RecordId::parse_simple(&user.id).map_err(|e| Error::BadRequest(e.to_string()))?;

    // Look up existing consent.
    let existing = consent_grant::get_for(&person_id, &client.id).await?;
    let needed = consent_grant::scopes_needing_consent(&existing, &requested_scopes);
    if !needed.is_empty() {
        return render_consent(client, &person_id, params, requested_scopes, needed, &user).await;
    }

    issue_code_and_redirect(&client, &person_id, params, requested_scopes).await
}

fn build_authorize_url(params: &AuthorizeParams) -> String {
    let mut u = format!(
        "/authorize?response_type={}&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method={}",
        urlencoding::encode(&params.response_type),
        urlencoding::encode(&params.client_id),
        urlencoding::encode(&params.redirect_uri),
        urlencoding::encode(&params.scope),
        urlencoding::encode(&params.code_challenge),
        urlencoding::encode(params.code_challenge_method.as_deref().unwrap_or("S256")),
    );
    if let Some(s) = &params.state {
        u.push_str(&format!("&state={}", urlencoding::encode(s)));
    }
    if let Some(n) = &params.nonce {
        u.push_str(&format!("&nonce={}", urlencoding::encode(n)));
    }
    u
}

async fn render_consent(
    client: OauthClient,
    person_id: &RecordId,
    params: AuthorizeParams,
    requested: Vec<String>,
    needed: Vec<String>,
    user: &std::sync::Arc<crate::middleware::CurrentUser>,
) -> Result<Response, Error> {
    let org_model = OrganizationModel::new();
    let org = org_model
        .get_by_id(&client.organization.to_raw_string())
        .await?;
    let already: Vec<String> = requested
        .iter()
        .filter(|s| !needed.contains(*s))
        .cloned()
        .collect();
    let scope_views: Vec<ScopeView> = needed
        .iter()
        .map(|s| ScopeView {
            scope: s.clone(),
            description: describe_scope(s, &org.name),
        })
        .collect();
    let _ = person_id; // silence unused if we move it later
    let mut base = BaseContext::new().with_page("oidc-consent");
    base = base.with_user(User::from_session_user(user).await);
    let template = ConsentTemplate {
        app_name: base.app_name,
        year: base.year,
        version: base.version,
        active_page: base.active_page,
        user: base.user,
        client_name: client.name,
        org_name: org.name,
        requested_scopes: scope_views,
        already_granted: already,
        params_json: serde_json::to_string(&params)
            .map_err(|e| Error::Internal(format!("serialize params: {e}")))?,
    };
    let html = template
        .render()
        .map_err(|e| Error::template(e.to_string()))?;
    Ok(Html(html).into_response())
}

#[derive(Debug, Deserialize)]
pub struct ConsentForm {
    pub params_json: String,
    pub action: String, // "approve" | "deny"
}

async fn consent(
    crate::middleware::AuthenticatedUser(user): crate::middleware::AuthenticatedUser,
    Form(form): Form<ConsentForm>,
) -> Result<Response, Error> {
    let params: AuthorizeParams = serde_json::from_str(&form.params_json)
        .map_err(|e| Error::BadRequest(format!("invalid params: {e}")))?;
    let model = OauthClientModel::new();
    let client = model
        .get_by_client_id(&params.client_id)
        .await?
        .ok_or_else(|| Error::BadRequest("unknown client_id".into()))?;
    let person_id =
        RecordId::parse_simple(&user.id).map_err(|e| Error::BadRequest(e.to_string()))?;

    if form.action != "approve" {
        let mut url = Url::parse(&params.redirect_uri)
            .map_err(|e| Error::BadRequest(format!("invalid redirect_uri: {e}")))?;
        url.query_pairs_mut().append_pair("error", "access_denied");
        if let Some(s) = &params.state {
            url.query_pairs_mut().append_pair("state", s);
        }
        return Ok(Redirect::to(url.as_str()).into_response());
    }

    let scopes: Vec<String> = params
        .scope
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    consent_grant::upsert_grant(&person_id, &client.id, &scopes).await?;
    issue_code_and_redirect(&client, &person_id, params, scopes).await
}

async fn issue_code_and_redirect(
    client: &OauthClient,
    person: &RecordId,
    params: AuthorizeParams,
    scopes: Vec<String>,
) -> Result<Response, Error> {
    let (code, _session_id) = oidc_tokens::create_authorization_code(CreateAuthorizationCode {
        client: &client.id,
        person,
        redirect_uri: &params.redirect_uri,
        scopes: &scopes,
        code_challenge: &params.code_challenge,
        code_challenge_method: params.code_challenge_method.as_deref().unwrap_or("S256"),
        nonce: params.nonce.as_deref(),
    })
    .await?;

    let mut url = Url::parse(&params.redirect_uri)
        .map_err(|e| Error::BadRequest(format!("invalid redirect_uri: {e}")))?;
    url.query_pairs_mut().append_pair("code", &code);
    if let Some(s) = &params.state {
        url.query_pairs_mut().append_pair("state", s);
    }
    Ok(Redirect::to(url.as_str()).into_response())
}

// ---------- /token ----------

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scope: Option<String>,
}

async fn token(headers: HeaderMap, Form(req): Form<TokenRequest>) -> Result<Response, Error> {
    let (client_id, client_secret) = extract_client_credentials(&headers, &req)?;
    let client_model = OauthClientModel::new();
    let client = client_model
        .get_by_client_id(&client_id)
        .await?
        .ok_or_else(|| token_error("invalid_client", "unknown client_id"))?;
    if !OauthClientModel::verify_secret(&client, &client_secret) {
        return Err(token_error(
            "invalid_client",
            "client authentication failed",
        ));
    }

    match req.grant_type.as_str() {
        "authorization_code" => handle_auth_code_grant(client, req).await,
        "refresh_token" => handle_refresh_grant(client, req).await,
        other => Err(token_error(
            "unsupported_grant_type",
            &format!("unsupported grant_type '{other}'"),
        )),
    }
}

fn extract_client_credentials(
    headers: &HeaderMap,
    req: &TokenRequest,
) -> Result<(String, String), Error> {
    if let Some(h) = headers.get(header::AUTHORIZATION) {
        let raw = h
            .to_str()
            .map_err(|_| token_error("invalid_request", "bad Authorization header"))?;
        if let Some(b64) = raw.strip_prefix("Basic ") {
            let decoded = STANDARD
                .decode(b64)
                .map_err(|_| token_error("invalid_request", "bad Basic auth"))?;
            let s = String::from_utf8(decoded)
                .map_err(|_| token_error("invalid_request", "bad Basic auth utf8"))?;
            let mut parts = s.splitn(2, ':');
            let id = parts
                .next()
                .ok_or_else(|| token_error("invalid_request", "missing client_id"))?;
            let secret = parts
                .next()
                .ok_or_else(|| token_error("invalid_request", "missing client_secret"))?;
            return Ok((
                urlencoding::decode(id).unwrap_or_default().into_owned(),
                urlencoding::decode(secret).unwrap_or_default().into_owned(),
            ));
        }
    }
    let id = req
        .client_id
        .clone()
        .ok_or_else(|| token_error("invalid_request", "client_id required"))?;
    let secret = req
        .client_secret
        .clone()
        .ok_or_else(|| token_error("invalid_request", "client_secret required"))?;
    Ok((id, secret))
}

fn token_error(error: &str, desc: &str) -> Error {
    let payload = json!({ "error": error, "error_description": desc });
    Error::BadRequest(payload.to_string())
}

async fn handle_auth_code_grant(client: OauthClient, req: TokenRequest) -> Result<Response, Error> {
    let code = req
        .code
        .ok_or_else(|| token_error("invalid_request", "code required"))?;
    let row = match oidc_tokens::consume_authorization_code(&code).await? {
        Some(r) => r,
        None => {
            // Disambiguate the failure for operator diagnostics. RP-facing
            // `error` field stays `invalid_grant` per RFC 6749 §5.2.
            let reason = match oidc_tokens::peek_authorization_code(&code).await? {
                Some(r) if r.consumed => "authorization code already used",
                Some(r) if r.expires_at <= chrono::Utc::now() => "authorization code expired",
                _ => "authorization code not found",
            };
            return Err(token_error("invalid_grant", reason));
        }
    };
    if row.client != client.id {
        return Err(token_error("invalid_grant", "code/client mismatch"));
    }
    let provided_redirect = req
        .redirect_uri
        .ok_or_else(|| token_error("invalid_request", "redirect_uri required"))?;
    if provided_redirect != row.redirect_uri {
        return Err(token_error("invalid_grant", "redirect_uri mismatch"));
    }
    if client.require_pkce {
        let verifier = req
            .code_verifier
            .ok_or_else(|| token_error("invalid_request", "code_verifier required"))?;
        if !oidc_tokens::verify_pkce_s256(&verifier, &row.code_challenge) {
            return Err(token_error("invalid_grant", "PKCE verification failed"));
        }
    }

    issue_token_response(
        &client,
        &row.person,
        &row.scopes,
        &row.session_id,
        row.nonce.as_deref(),
    )
    .await
}

async fn handle_refresh_grant(client: OauthClient, req: TokenRequest) -> Result<Response, Error> {
    let token = req
        .refresh_token
        .ok_or_else(|| token_error("invalid_request", "refresh_token required"))?;
    let row = oidc_tokens::lookup_refresh_token(&token)
        .await?
        .ok_or_else(|| token_error("invalid_grant", "refresh_token invalid or expired"))?;
    if row.client != client.id {
        return Err(token_error(
            "invalid_grant",
            "refresh_token/client mismatch",
        ));
    }

    // Reuse-detection: if this row was already revoked, kill the entire chain.
    if row.revoked_at.is_some() {
        let _ = oidc_tokens::revoke_session(&row.session_id).await;
        return Err(token_error("invalid_grant", "refresh_token reuse detected"));
    }

    // Optional scope downscoping
    let mut new_scopes = row.scopes.clone();
    if let Some(req_scope) = req.scope {
        let requested: Vec<String> = req_scope
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        for s in &requested {
            if !row.scopes.contains(s) {
                return Err(token_error("invalid_scope", "scope must be subset"));
            }
        }
        new_scopes = requested;
    }

    // Rotate the refresh token: revoke old, issue new with same session_id.
    oidc_tokens::revoke_refresh_token(&row.id).await?;
    issue_token_response(&client, &row.person, &new_scopes, &row.session_id, None).await
}

async fn issue_token_response(
    client: &OauthClient,
    person: &RecordId,
    scopes: &[String],
    session_id: &str,
    nonce: Option<&str>,
) -> Result<Response, Error> {
    let want_refresh = scopes.iter().any(|s| s == "offline_access");
    let refresh_pair = if want_refresh {
        Some(
            oidc_tokens::create_refresh_token(CreateRefreshToken {
                client: &client.id,
                person,
                scopes,
                session_id,
                rotated_from: None,
            })
            .await?,
        )
    } else {
        None
    };
    let access_token = oidc_tokens::create_access_token(CreateAccessToken {
        client: &client.id,
        person,
        scopes,
        session_id,
        refresh_token: refresh_pair.as_ref().map(|(_, id)| id),
    })
    .await?;
    let id_token = build_id_token(client, person, scopes, nonce).await?;

    let mut body = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": ACCESS_TOKEN_TTL_SECONDS,
        "id_token": id_token,
        "scope": scopes.join(" "),
    });
    if let Some((rt, _)) = refresh_pair {
        body["refresh_token"] = json!(rt);
    }
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
        .into_response())
}

async fn build_id_token(
    client: &OauthClient,
    person: &RecordId,
    scopes: &[String],
    nonce: Option<&str>,
) -> Result<String, Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let exp = now + 600; // id_token lives 10 minutes — clients should treat access_token as session
    let mut claims = serde_json::Map::new();
    claims.insert("iss".into(), json!(config::app_url()));
    claims.insert("sub".into(), json!(person.to_raw_string()));
    claims.insert("aud".into(), json!(client.client_id));
    claims.insert("iat".into(), json!(now));
    claims.insert("exp".into(), json!(exp));
    claims.insert("auth_time".into(), json!(now));
    if let Some(n) = nonce {
        claims.insert("nonce".into(), json!(n));
    }

    enrich_userinfo_claims(client, person, scopes, &mut claims).await?;

    oidc_keys::sign_id_token(&Value::Object(claims)).await
}

async fn enrich_userinfo_claims(
    client: &OauthClient,
    person: &RecordId,
    scopes: &[String],
    out: &mut serde_json::Map<String, Value>,
) -> Result<(), Error> {
    if scopes.iter().any(|s| s == "profile" || s == "email") {
        let p = Person::find_by_id(&person.to_raw_string())
            .await?
            .ok_or(Error::NotFound)?;
        if scopes.iter().any(|s| s == "profile") {
            out.insert("preferred_username".into(), json!(p.username.clone()));
            if let Some(name) = p.profile.as_ref().and_then(|pr| pr.name.clone()) {
                out.insert("name".into(), json!(name));
            } else if let Some(name) = p.name.clone() {
                out.insert("name".into(), json!(name));
            }
        }
        if scopes.iter().any(|s| s == "email") {
            out.insert("email".into(), json!(p.email.clone()));
            out.insert(
                "email_verified".into(),
                json!(p.verification_status != "unverified"),
            );
        }
    }
    if scopes.iter().any(|s| s == "slatehub:org_membership") {
        let org_model = OrganizationModel::new();
        let org = org_model
            .get_by_id(&client.organization.to_raw_string())
            .await?;
        let role = org_model
            .get_member_role(
                &client.organization.to_raw_string(),
                &person.to_raw_string(),
            )
            .await?;
        out.insert(
            "slatehub_org".into(),
            json!({
                "id": client.organization.to_raw_string(),
                "slug": org.slug,
                "name": org.name,
            }),
        );
        out.insert("slatehub_org_role".into(), json!(role));
        // permissions array — fetch from the member_of edge
        let perms = fetch_member_permissions(&client.organization, person).await?;
        out.insert("slatehub_permissions".into(), json!(perms));
    }
    Ok(())
}

async fn fetch_member_permissions(org: &RecordId, person: &RecordId) -> Result<Vec<String>, Error> {
    use crate::db::DB;
    #[derive(serde::Deserialize, serde::Serialize, SurrealValue)]
    struct Row {
        permissions: Vec<String>,
    }
    let mut resp = DB
        .query(
            "SELECT permissions FROM member_of \
             WHERE in = $person AND out = $org AND invitation_status = 'accepted' LIMIT 1",
        )
        .bind(("person", person.clone()))
        .bind(("org", org.clone()))
        .await?;
    let rows: Vec<Row> = resp.take(0).unwrap_or_default();
    Ok(rows
        .into_iter()
        .next()
        .map(|r| r.permissions)
        .unwrap_or_default())
}

// ---------- /userinfo ----------

async fn userinfo(headers: HeaderMap) -> Result<Response, Error> {
    let token = bearer_token(&headers).ok_or(Error::Unauthorized)?;
    let row = oidc_tokens::lookup_access_token(&token)
        .await?
        .ok_or(Error::Unauthorized)?;

    let client = OauthClientModel::new()
        .get_by_client_id(&{
            let mut resp = crate::db::DB
                .query("SELECT <string> client_id AS client_id FROM $id")
                .bind(("id", row.client.clone()))
                .await?;
            #[derive(serde::Deserialize, serde::Serialize, SurrealValue)]
            struct C {
                client_id: String,
            }
            let v: Vec<C> = resp.take(0).unwrap_or_default();
            v.into_iter()
                .next()
                .map(|c| c.client_id)
                .unwrap_or_default()
        })
        .await?
        .ok_or(Error::Unauthorized)?;

    let mut out = serde_json::Map::new();
    out.insert("sub".into(), json!(row.person.to_raw_string()));
    enrich_userinfo_claims(&client, &row.person, &row.scopes, &mut out).await?;
    Ok((
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(Value::Object(out)),
    )
        .into_response())
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    raw.strip_prefix("Bearer ").map(|s| s.to_string())
}

// ---------- /revoke ----------

#[derive(Debug, Deserialize)]
pub struct RevokeForm {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

async fn revoke(headers: HeaderMap, Form(form): Form<RevokeForm>) -> Result<Response, Error> {
    // Authenticate the client (RFC 7009). Always return 200 regardless of token validity.
    let req = TokenRequest {
        grant_type: "".into(),
        code: None,
        redirect_uri: None,
        code_verifier: None,
        refresh_token: None,
        client_id: form.client_id.clone(),
        client_secret: form.client_secret.clone(),
        scope: None,
    };
    let (cid, csecret) = extract_client_credentials(&headers, &req)?;
    let client = OauthClientModel::new()
        .get_by_client_id(&cid)
        .await?
        .ok_or_else(|| token_error("invalid_client", "unknown client_id"))?;
    if !OauthClientModel::verify_secret(&client, &csecret) {
        return Err(token_error(
            "invalid_client",
            "client authentication failed",
        ));
    }

    let hint = form.token_type_hint.as_deref();
    if hint != Some("access_token")
        && let Ok(Some(rt)) = oidc_tokens::lookup_refresh_token(&form.token).await
        && rt.client == client.id
    {
        let _ = oidc_tokens::revoke_session(&rt.session_id).await;
        return Ok(StatusCode::OK.into_response());
    }
    let hash = oidc_tokens::sha256_hex(&form.token);
    let _ = oidc_tokens::revoke_access_token_by_hash(&hash).await;
    Ok(StatusCode::OK.into_response())
}

// ---------- /introspect ----------

#[derive(Debug, Deserialize)]
pub struct IntrospectForm {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

async fn introspect(
    headers: HeaderMap,
    Form(form): Form<IntrospectForm>,
) -> Result<Response, Error> {
    let req = TokenRequest {
        grant_type: "".into(),
        code: None,
        redirect_uri: None,
        code_verifier: None,
        refresh_token: None,
        client_id: form.client_id.clone(),
        client_secret: form.client_secret.clone(),
        scope: None,
    };
    let (cid, csecret) = extract_client_credentials(&headers, &req)?;
    let client = OauthClientModel::new()
        .get_by_client_id(&cid)
        .await?
        .ok_or_else(|| token_error("invalid_client", "unknown client_id"))?;
    if !OauthClientModel::verify_secret(&client, &csecret) {
        return Err(token_error(
            "invalid_client",
            "client authentication failed",
        ));
    }

    let hint = form.token_type_hint.as_deref();
    if hint != Some("refresh_token")
        && let Ok(Some(at)) = oidc_tokens::lookup_access_token(&form.token).await
        && at.client == client.id
    {
        return Ok(Json(json!({
            "active": true,
            "scope": at.scopes.join(" "),
            "client_id": cid,
            "exp": at.expires_at.timestamp(),
            "sub": at.person.to_raw_string(),
            "token_type": "Bearer",
            "session_id": at.session_id,
        }))
        .into_response());
    }
    if let Ok(Some(rt)) = oidc_tokens::lookup_refresh_token(&form.token).await
        && rt.client == client.id
    {
        return Ok(Json(json!({
            "active": true,
            "scope": rt.scopes.join(" "),
            "client_id": cid,
            "exp": rt.expires_at.timestamp(),
            "sub": rt.person.to_raw_string(),
            "token_type": "refresh_token",
            "session_id": rt.session_id,
        }))
        .into_response());
    }
    Ok(Json(json!({ "active": false })).into_response())
}

// ---------- /logout (RP-initiated) ----------

#[derive(Debug, Deserialize)]
pub struct LogoutParams {
    pub id_token_hint: Option<String>,
    pub post_logout_redirect_uri: Option<String>,
    pub state: Option<String>,
    pub client_id: Option<String>,
}

async fn rp_initiated_logout(
    Query(p): Query<LogoutParams>,
    jar: axum_extra::extract::CookieJar,
) -> Result<Response, Error> {
    use axum_extra::extract::cookie::Cookie;
    use cookie::SameSite;

    // If a post-logout URI is requested, validate it against the client's allowlist.
    let mut validated_redirect: Option<String> = None;
    if let Some(uri) = p.post_logout_redirect_uri.clone() {
        let cid = match (p.client_id.clone(), p.id_token_hint.clone()) {
            (Some(c), _) => Some(c),
            (None, Some(_hint)) => {
                // Decode the audience claim from the hint without verifying signature.
                // (Optional best-effort — we still check the URI allowlist below.)
                None
            }
            _ => None,
        };
        if let Some(cid) = cid
            && let Ok(Some(client)) = OauthClientModel::new().get_by_client_id(&cid).await
        {
            if client.post_logout_redirect_uris.iter().any(|u| u == &uri) {
                validated_redirect = Some(uri);
            } else {
                warn!(
                    client_id = %cid,
                    "Rejecting post_logout_redirect_uri not in allowlist"
                );
            }
        }
    }

    // Kill the local SlateHub session cookie.
    let secure = std::env::var("COOKIE_SECURE").unwrap_or_else(|_| "true".to_string()) != "false";
    let cookie = Cookie::build(("auth_token", ""))
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(secure)
        .max_age(Default::default())
        .build();
    let jar = jar.remove(cookie);

    let target = match validated_redirect {
        Some(uri) => {
            let mut u = Url::parse(&uri)
                .map_err(|e| Error::BadRequest(format!("invalid post_logout_redirect_uri: {e}")))?;
            if let Some(s) = &p.state {
                u.query_pairs_mut().append_pair("state", s);
            }
            u.into()
        }
        None => "/".to_string(),
    };
    Ok((jar, Redirect::to(&target)).into_response())
}
