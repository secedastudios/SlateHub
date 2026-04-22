#[allow(dead_code)]
mod common;

use chrono::{Duration, Utc};
use slatehub::db::DB;
use slatehub::services::oidc_tokens::{
    AUTHORIZATION_CODE_TTL_SECONDS, consume_authorization_code, peek_authorization_code,
};
use surrealdb::types::SurrealValue;

#[derive(serde::Deserialize, SurrealValue)]
struct ConsumedRow {
    consumed: bool,
}

async fn seed_org_and_person() -> (String, String) {
    #[derive(serde::Deserialize, SurrealValue)]
    struct Id {
        id: String,
    }

    let mut p = DB
        .query(
            "CREATE person CONTENT {
                email: $email,
                password: 'hashed',
                username: $username,
                profile: { name: $username, skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
            } RETURN string::concat('person:', meta::id(id)) AS id",
        )
        .bind(("email", "tok-test@example.com".to_string()))
        .bind(("username", "tok-test-user".to_string()))
        .await
        .expect("create person");
    let person: Vec<Id> = p.take(0).expect("take person");
    let person_id = person[0].id.clone();

    let mut org_type_resp = DB
        .query("SELECT string::concat('organization_type:', meta::id(id)) AS id FROM organization_type LIMIT 1")
        .await
        .expect("query org type");
    let org_type: Vec<Id> = org_type_resp.take(0).expect("take org type");
    assert!(
        !org_type.is_empty(),
        "No organization_type rows seeded — run `make test-db-init`."
    );
    let org_type_id = org_type[0].id.clone();

    let mut o = DB
        .query(
            "CREATE organization CONTENT {
                name: 'Tok Test Org',
                slug: $slug,
                type: type::record('organization_type', $type_key),
                services: [],
                public: true
            } RETURN string::concat('organization:', meta::id(id)) AS id",
        )
        .bind(("slug", "tok-test-org".to_string()))
        .bind((
            "type_key",
            org_type_id
                .strip_prefix("organization_type:")
                .unwrap()
                .to_string(),
        ))
        .await
        .expect("create org");
    let org: Vec<Id> = o.take(0).expect("take org");
    let org_id = org[0].id.clone();

    let mut c = DB
        .query(
            "CREATE oauth_client CONTENT {
                organization: type::record('organization', $org_key),
                client_id: 'sh_test_client',
                client_secret_hash: 'placeholder',
                name: 'Tok Test Client',
                redirect_uris: ['http://127.0.0.1:3000/cb'],
                post_logout_redirect_uris: [],
                allowed_scopes: ['openid', 'profile'],
                token_endpoint_auth_method: 'client_secret_basic',
                require_pkce: true,
                ssf_delivery_method: 'push',
                ssf_events_subscribed: []
            } RETURN string::concat('oauth_client:', meta::id(id)) AS id",
        )
        .bind((
            "org_key",
            org_id.strip_prefix("organization:").unwrap().to_string(),
        ))
        .await
        .expect("create client");
    let client: Vec<Id> = c.take(0).expect("take client");
    (client[0].id.clone(), person_id)
}

async fn create_code(
    client_id: &str,
    person_id: &str,
    code: &str,
    consumed: bool,
    expires_offset_secs: i64,
) {
    let expires_at = Utc::now() + Duration::seconds(expires_offset_secs);
    DB.query(
        "CREATE authorization_code CONTENT {
            code: $code,
            client: type::record('oauth_client', $client_key),
            person: type::record('person', $person_key),
            redirect_uri: 'http://127.0.0.1:3000/cb',
            scopes: ['openid'],
            code_challenge: 'dummy',
            code_challenge_method: 'S256',
            nonce: NONE,
            session_id: 'sid-test',
            expires_at: $exp,
            consumed: $consumed
        } RETURN NONE",
    )
    .bind(("code", code.to_string()))
    .bind((
        "client_key",
        client_id.strip_prefix("oauth_client:").unwrap().to_string(),
    ))
    .bind((
        "person_key",
        person_id.strip_prefix("person:").unwrap().to_string(),
    ))
    .bind(("exp", expires_at))
    .bind(("consumed", consumed))
    .await
    .expect("create authorization_code");
}

async fn cleanup() {
    let _ = DB.query("DELETE authorization_code").await;
    let _ = DB.query("DELETE oauth_client").await;
    let _ = DB.query("DELETE organization").await;
    let _ = DB.query("DELETE person").await;
}

async fn is_consumed(code: &str) -> bool {
    let mut resp = DB
        .query("SELECT consumed FROM authorization_code WHERE code = $code LIMIT 1")
        .bind(("code", code.to_string()))
        .await
        .expect("select authorization_code");
    let rows: Vec<ConsumedRow> = resp.take(0).expect("take");
    rows.into_iter().next().expect("row exists").consumed
}

#[test]
fn test_consume_happy_path() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-happy", false, 60).await;

        let row = consume_authorization_code("code-happy")
            .await
            .expect("consume call");
        let row = row.expect("Some(row) on first consume");
        // RETURN BEFORE: pre-update value should be `consumed = false`.
        assert!(!row.consumed, "pre-update row reports consumed = false");

        // Side-effect persisted: subsequent peek shows consumed = true.
        assert!(is_consumed("code-happy").await);

        cleanup().await;
    });
}

#[test]
fn test_consume_twice_returns_none() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-twice", false, 60).await;

        let first = consume_authorization_code("code-twice")
            .await
            .expect("first consume");
        assert!(first.is_some(), "first consume returns Some");

        let second = consume_authorization_code("code-twice")
            .await
            .expect("second consume");
        assert!(second.is_none(), "second consume must return None");

        cleanup().await;
    });
}

#[test]
fn test_consume_already_consumed_returns_none() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-already", true, 60).await;

        let result = consume_authorization_code("code-already")
            .await
            .expect("consume");
        assert!(result.is_none());
        // Row was already consumed and stays consumed (UPDATE WHERE excluded it).
        assert!(is_consumed("code-already").await);

        cleanup().await;
    });
}

#[test]
fn test_consume_expired_returns_none() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-expired", false, -10).await;

        let result = consume_authorization_code("code-expired")
            .await
            .expect("consume");
        assert!(result.is_none());
        // Row stays unconsumed — UPDATE WHERE clause excluded it.
        assert!(!is_consumed("code-expired").await);

        cleanup().await;
    });
}

#[test]
fn test_authorization_code_ttl_is_300_seconds() {
    // 5 minutes — Google/Microsoft default. Single-use enforcement is the
    // replay protection; the TTL just shouldn't punish honest clients.
    assert_eq!(AUTHORIZATION_CODE_TTL_SECONDS, 300);
}

// ---------- peek_authorization_code: disambiguate the three failure modes ----------

#[test]
fn test_peek_returns_none_for_unknown_code() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let row = peek_authorization_code("does-not-exist")
            .await
            .expect("peek");
        assert!(row.is_none(), "unknown code returns None");
    });
}

#[test]
fn test_peek_reports_consumed_for_already_used_code() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-peek-used", true, 60).await;

        let row = peek_authorization_code("code-peek-used")
            .await
            .expect("peek")
            .expect("Some(row)");
        assert!(row.consumed, "peek surfaces consumed = true");
        assert!(
            row.expires_at > Utc::now(),
            "and expires_at is still in the future"
        );

        cleanup().await;
    });
}

#[test]
fn test_peek_reports_expired_for_expired_code() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-peek-expired", false, -10).await;

        let row = peek_authorization_code("code-peek-expired")
            .await
            .expect("peek")
            .expect("Some(row)");
        assert!(!row.consumed, "peek reports consumed = false");
        assert!(
            row.expires_at <= Utc::now(),
            "and expires_at is in the past"
        );

        cleanup().await;
    });
}

#[test]
fn test_peek_does_not_mutate_row() {
    common::setup_test_db();
    common::run(async {
        cleanup().await;
        let (client_id, person_id) = seed_org_and_person().await;
        create_code(&client_id, &person_id, "code-peek-readonly", false, 60).await;

        let _ = peek_authorization_code("code-peek-readonly").await.unwrap();
        assert!(
            !is_consumed("code-peek-readonly").await,
            "peek must not flip consumed"
        );

        cleanup().await;
    });
}
