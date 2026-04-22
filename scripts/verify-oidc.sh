#!/usr/bin/env bash
# End-to-end OIDC verification.
#
# Walks the full Authorization Code + PKCE flow against a running Slatehub
# server and asserts every endpoint returns what it should. If this script
# exits 0, the OIDC provider is functional end-to-end.
#
# Usage:
#   make oidc-verify                 # uses defaults below
#   ISSUER=https://slatehub.com scripts/verify-oidc.sh
#
# Requirements: curl, jq, openssl, python3 (for PKCE + JWT decode), surreal CLI
# (or docker exec into slatehub-surrealdb), and a running Slatehub server.

set -euo pipefail

ISSUER="${ISSUER:-http://localhost:3000}"
DB_HTTP="${DB_HTTP:-http://localhost:8000}"
DB_USER="${DB_USER:-root}"
DB_PASS="${DB_PASS:-root}"
DB_NS="${DB_NAMESPACE:-slatehub}"
DB_NAME="${DB_NAME:-main}"

# Test fixtures we'll create + clean up.
TEST_ORG_SLUG="oidc-verify-org-$$"
TEST_REDIRECT="http://127.0.0.1:9999/cb"
TEST_USER_EMAIL="oidc-verify-$$@example.com"
TEST_USER_PASS="oidc-verify-pw-$$"
TEST_USER_NAME="oidcverify$$"

RED=$'\e[31m'; GREEN=$'\e[32m'; YEL=$'\e[33m'; CYAN=$'\e[36m'; NC=$'\e[0m'

step() { printf "%s▶ %s%s\n" "$CYAN" "$1" "$NC"; }
ok()   { printf "  %s✓ %s%s\n" "$GREEN" "$1" "$NC"; }
fail() { printf "  %s✗ %s%s\n" "$RED" "$1" "$NC"; exit 1; }

# Last raw DB response — dumped on unexpected exit so we can see what we got.
LAST_SURQL_RESPONSE=""
on_err() {
  local rc=$?
  printf "\n%s✗ unexpected exit (rc=%d)%s\n" "$RED" "$rc" "$NC"
  if [ -n "$LAST_SURQL_RESPONSE" ]; then
    printf "%slast surql response:%s\n%s\n" "$YEL" "$NC" "$LAST_SURQL_RESPONSE"
  fi
}
trap on_err ERR

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "${RED}missing dep: $1${NC}"; exit 2; }
}

# Run SurrealQL via the HTTP /sql endpoint. Returns the raw JSON response —
# always an array of `{status, result, time}` objects, one per statement. The
# CLI shell pollutes stdout with a welcome banner, so HTTP is the only
# reliable way to script this.
surql() {
  # No -f so that 4xx/5xx response bodies still come back in
  # LAST_SURQL_RESPONSE (so on_err can dump them).
  LAST_SURQL_RESPONSE=$(curl -s -X POST "$DB_HTTP/sql" \
    -H "Accept: application/json" \
    -H "surreal-ns: $DB_NS" \
    -H "surreal-db: $DB_NAME" \
    -u "$DB_USER:$DB_PASS" \
    --data-binary "$1")
  # Detect surql per-statement failure too (status != "OK").
  if echo "$LAST_SURQL_RESPONSE" | jq -e '.[]?.status | select(. != "OK")' >/dev/null 2>&1; then
    printf "  %ssurql statement failed: %s%s\n" "$RED" \
      "$(echo "$LAST_SURQL_RESPONSE" | jq -c '.')" "$NC" >&2
    return 1
  fi
  printf '%s' "$LAST_SURQL_RESPONSE"
}

require curl
require jq
require openssl
require python3

# Auto-manage a venv with pynacl (needed for EdDSA id_token signature
# verification). PEP 668 blocks system-wide pip on most modern setups; a venv
# sidesteps it. First run creates + installs (~5s), later runs are instant.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv-oidc-verify"
VENV_PY="$VENV_DIR/bin/python3"
if ! "$VENV_PY" -c 'import nacl.signing' >/dev/null 2>&1; then
  step "Setting up Python venv (one-time, installs pynacl)"
  python3 -m venv "$VENV_DIR" >/dev/null \
    || fail "could not create venv at $VENV_DIR"
  "$VENV_PY" -m pip install --quiet --upgrade pip pynacl \
    || fail "pip install pynacl failed in $VENV_DIR"
  ok "venv ready at $VENV_DIR"
fi
PY="$VENV_PY"

cleanup() {
  step "Cleanup"
  surql "
    LET \$org = (SELECT VALUE id FROM organization WHERE slug = '$TEST_ORG_SLUG' LIMIT 1)[0];
    IF \$org != NONE THEN
      DELETE consent_grant WHERE out IN (SELECT VALUE id FROM oauth_client WHERE organization = \$org);
      DELETE access_token WHERE client IN (SELECT VALUE id FROM oauth_client WHERE organization = \$org);
      DELETE refresh_token WHERE client IN (SELECT VALUE id FROM oauth_client WHERE organization = \$org);
      DELETE authorization_code WHERE client IN (SELECT VALUE id FROM oauth_client WHERE organization = \$org);
      DELETE oauth_client WHERE organization = \$org;
      DELETE member_of WHERE out = \$org;
      DELETE \$org
    END;
    DELETE person WHERE email = '$TEST_USER_EMAIL';
  " >/dev/null 2>&1 || true
  ok "removed test fixtures"
}
trap cleanup EXIT

# ---------- 0. ping the server + DB --------------------------------------
step "Preflight"
curl -sf "$ISSUER/.well-known/openid-configuration" >/dev/null \
  || fail "server not reachable at $ISSUER"
ok "server alive"

surql "INFO FOR DB" >/dev/null || fail "DB not reachable at $DB_HTTP"
ok "db alive"

# ---------- 1. /.well-known/openid-configuration -------------------------
step "Discovery"
DISCOVERY=$(curl -sf "$ISSUER/.well-known/openid-configuration")
for field in issuer authorization_endpoint token_endpoint userinfo_endpoint \
             jwks_uri revocation_endpoint introspection_endpoint \
             end_session_endpoint; do
  echo "$DISCOVERY" | jq -er ".$field" >/dev/null \
    || fail "discovery missing $field"
done
ok "discovery contains all required endpoints"

# ---------- 2. JWKS ------------------------------------------------------
step "JWKS"
JWKS=$(curl -sf "$ISSUER/.well-known/jwks.json")
N_KEYS=$(echo "$JWKS" | jq -r '.keys | length')
[ "$N_KEYS" -ge 1 ] || fail "JWKS has zero keys (signing key missing)"
KID=$(echo "$JWKS" | jq -r '.keys[0].kid')
ok "JWKS has $N_KEYS key(s); active kid=$KID"

# ---------- 3. bootstrap test fixtures ------------------------------------
cleanup >/dev/null 2>&1 || true
step "Bootstrap test user + org + oauth_client"

# Hash the test password with the server's argon2 helper.
PASS_HASH=$(surql "RETURN crypto::argon2::generate('$TEST_USER_PASS')" \
  | jq -r '.[0].result')
[ -n "$PASS_HASH" ] && [ "$PASS_HASH" != "null" ] \
  || fail "could not hash password via surreal crypto helper"

PERSON_ID=$(surql "
  CREATE person CONTENT {
    email: '$TEST_USER_EMAIL',
    password: '$PASS_HASH',
    username: '$TEST_USER_NAME',
    profile: { name: 'OIDC Verify', skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
  } RETURN string::concat('person:', meta::id(id)) AS id
" | jq -r '.[0].result[0].id')
[ -n "$PERSON_ID" ] && [ "$PERSON_ID" != "null" ] || fail "person create failed"
ok "created person $PERSON_ID"

# Force email-verified so login isn't blocked.
surql "UPDATE $PERSON_ID SET verification_status = 'email'" >/dev/null

ORG_TYPE_RESP=$(surql "SELECT string::concat('organization_type:', meta::id(id)) AS id FROM organization_type LIMIT 1")
ORG_TYPE=$(echo "$ORG_TYPE_RESP" | jq -r '.[0].result[0].id')
[ -n "$ORG_TYPE" ] && [ "$ORG_TYPE" != "null" ] \
  || fail "no organization_type rows seeded — run make db-init (raw: $ORG_TYPE_RESP)"

ORG_ID=$(surql "
  CREATE organization CONTENT {
    name: 'OIDC Verify Org',
    slug: '$TEST_ORG_SLUG',
    type: $ORG_TYPE,
    services: [],
    social_links: [],
    public: true
  } RETURN string::concat('organization:', meta::id(id)) AS id
" | jq -r '.[0].result[0].id')
[ -n "$ORG_ID" ] && [ "$ORG_ID" != "null" ] || fail "org create failed"
ok "created org $ORG_ID"

surql "RELATE $PERSON_ID->member_of->$ORG_ID SET role = 'owner', invitation_status = 'accepted'" \
  >/dev/null
ok "made user owner of org"

# Create client directly (avoid having to log in to enable it via UI).
CLIENT_ID="sh_verify_$$"
CLIENT_SECRET="verify_secret_$$_$(openssl rand -hex 16)"
SECRET_HASH=$(surql "RETURN crypto::argon2::generate('$CLIENT_SECRET')" \
  | jq -r '.[0].result')
[ -n "$SECRET_HASH" ] && [ "$SECRET_HASH" != "null" ] \
  || fail "could not hash client secret"
CLIENT_RID=$(surql "
  CREATE oauth_client CONTENT {
    organization: $ORG_ID,
    client_id: '$CLIENT_ID',
    client_secret_hash: '$SECRET_HASH',
    name: 'OIDC Verify Client',
    redirect_uris: ['$TEST_REDIRECT'],
    post_logout_redirect_uris: [],
    allowed_scopes: ['openid', 'profile', 'email', 'offline_access', 'slatehub:org_membership'],
    token_endpoint_auth_method: 'client_secret_basic',
    require_pkce: true,
    ssf_delivery_method: 'push',
    ssf_events_subscribed: []
  } RETURN string::concat('oauth_client:', meta::id(id)) AS id
" | jq -r '.[0].result[0].id')
[ -n "$CLIENT_RID" ] && [ "$CLIENT_RID" != "null" ] || fail "client create failed"
ok "created oauth_client $CLIENT_RID (client_id=$CLIENT_ID)"

# ---------- 4. log in as the test user, capture auth_token cookie --------
step "Login"
COOKIE_JAR=$(mktemp)
trap 'rm -f "$COOKIE_JAR"; cleanup' EXIT

LOGIN_STATUS=$(curl -sf -o /dev/null -w '%{http_code}' \
  -c "$COOKIE_JAR" \
  -X POST "$ISSUER/login" \
  --data-urlencode "email=$TEST_USER_EMAIL" \
  --data-urlencode "password=$TEST_USER_PASS" \
  -H 'Accept: text/html')
case "$LOGIN_STATUS" in
  200|303|302) ok "login OK ($LOGIN_STATUS)" ;;
  *) fail "login returned $LOGIN_STATUS" ;;
esac
grep -q auth_token "$COOKIE_JAR" || fail "no auth_token cookie set"
ok "auth_token cookie present"

# ---------- 5. PKCE: generate verifier + S256 challenge -------------------
step "PKCE"
PKCE=$(python3 - <<'PY'
import base64, hashlib, secrets
v = base64.urlsafe_b64encode(secrets.token_bytes(32)).rstrip(b'=').decode()
c = base64.urlsafe_b64encode(hashlib.sha256(v.encode()).digest()).rstrip(b'=').decode()
print(f"{v} {c}")
PY
)
PKCE_VERIFIER=${PKCE% *}
PKCE_CHALLENGE=${PKCE#* }
STATE="state-$$"
NONCE="nonce-$$"
ok "verifier=${PKCE_VERIFIER:0:10}…  challenge=${PKCE_CHALLENGE:0:10}…"

# ---------- 6. /authorize → consent screen --------------------------------
step "/authorize (expect consent screen)"
AUTHZ_URL="$ISSUER/authorize?response_type=code&client_id=$CLIENT_ID"
AUTHZ_URL+="&redirect_uri=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$TEST_REDIRECT', safe=''))")"
AUTHZ_URL+="&scope=openid+profile+email+slatehub%3Aorg_membership"
AUTHZ_URL+="&code_challenge=$PKCE_CHALLENGE&code_challenge_method=S256"
AUTHZ_URL+="&state=$STATE&nonce=$NONCE"

CONSENT_HTML=$(curl -sf -b "$COOKIE_JAR" -c "$COOKIE_JAR" "$AUTHZ_URL")
echo "$CONSENT_HTML" | grep -q 'name="params_json"' \
  || fail "no consent screen rendered (missing params_json)"
PARAMS_JSON=$(echo "$CONSENT_HTML" \
  | grep -oE 'name="params_json" value="[^"]*"' \
  | sed -E 's/name="params_json" value="(.*)"/\1/' \
  | python3 -c 'import sys,html;print(html.unescape(sys.stdin.read()))')
[ -n "$PARAMS_JSON" ] || fail "could not extract params_json from consent form"
ok "consent screen rendered, captured params_json"

# ---------- 7. POST /authorize/consent (Approve) -> 302 with code --------
step "POST /authorize/consent (approve) → expect 302 to redirect_uri with code"
CALLBACK_URL=$(curl -s -o /dev/null -w '%{redirect_url}' \
  -b "$COOKIE_JAR" -c "$COOKIE_JAR" \
  -X POST "$ISSUER/authorize/consent" \
  --data-urlencode "params_json=$PARAMS_JSON" \
  --data-urlencode "action=approve")
case "$CALLBACK_URL" in
  "$TEST_REDIRECT"\?*) ok "redirected to $CALLBACK_URL" ;;
  *) fail "expected redirect to $TEST_REDIRECT?…, got '$CALLBACK_URL'" ;;
esac

CODE=$(python3 - <<PY
import urllib.parse
q = urllib.parse.urlparse("$CALLBACK_URL").query
p = dict(urllib.parse.parse_qsl(q))
print(p.get("code", ""))
print(p.get("state", ""))
PY
)
RECEIVED_CODE=$(echo "$CODE" | head -n1)
RECEIVED_STATE=$(echo "$CODE" | tail -n1)
[ -n "$RECEIVED_CODE" ] || fail "no code in redirect"
[ "$RECEIVED_STATE" = "$STATE" ] || fail "state mismatch (got '$RECEIVED_STATE', want '$STATE')"
ok "got code + matching state"

# ---------- 8. POST /token (authorization_code grant) --------------------
step "POST /token (authorization_code grant)"
TOKEN_RESP=$(curl -sf -X POST "$ISSUER/token" \
  -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "grant_type=authorization_code" \
  --data-urlencode "code=$RECEIVED_CODE" \
  --data-urlencode "redirect_uri=$TEST_REDIRECT" \
  --data-urlencode "code_verifier=$PKCE_VERIFIER")

ACCESS_TOKEN=$(echo "$TOKEN_RESP" | jq -r '.access_token')
ID_TOKEN=$(echo "$TOKEN_RESP" | jq -r '.id_token')
TOKEN_TYPE=$(echo "$TOKEN_RESP" | jq -r '.token_type')
EXPIRES_IN=$(echo "$TOKEN_RESP" | jq -r '.expires_in')
SCOPES_OUT=$(echo "$TOKEN_RESP" | jq -r '.scope')
[ "$ACCESS_TOKEN" != "null" ] || fail "no access_token in /token response: $TOKEN_RESP"
[ "$ID_TOKEN" != "null" ] || fail "no id_token in /token response"
[ "$TOKEN_TYPE" = "Bearer" ] || fail "expected token_type=Bearer, got '$TOKEN_TYPE'"
[ "$EXPIRES_IN" -gt 0 ] || fail "non-positive expires_in"
case "$SCOPES_OUT" in
  *openid*) ok "tokens issued (expires_in=${EXPIRES_IN}s, scopes=$SCOPES_OUT)" ;;
  *) fail "scope missing 'openid' in token response: $SCOPES_OUT" ;;
esac

# ---------- 9. verify id_token signature against JWKS ---------------------
step "id_token signature verification (EdDSA via JWKS)"
"$PY" - <<PY || fail "id_token signature verification failed"
import base64, json, hashlib, sys, urllib.request
import nacl.signing  # PyNaCl
b64u = lambda b: base64.urlsafe_b64encode(b).rstrip(b'=')
def b64ud(s):
    return base64.urlsafe_b64decode(s + '=' * (-len(s) % 4))

jwks = json.loads(urllib.request.urlopen("$ISSUER/.well-known/jwks.json").read())
tok = "$ID_TOKEN"
h_b64, p_b64, s_b64 = tok.split('.')
header = json.loads(b64ud(h_b64))
payload = json.loads(b64ud(p_b64))
sig = b64ud(s_b64)
kid = header['kid']
jwk = next(k for k in jwks['keys'] if k['kid'] == kid)
pub = b64ud(jwk['x'])
nacl.signing.VerifyKey(pub).verify(f"{h_b64}.{p_b64}".encode(), sig)
assert payload['iss'] == "$ISSUER", f"iss mismatch: {payload['iss']}"
assert payload['aud'] == "$CLIENT_ID", f"aud mismatch: {payload['aud']}"
assert payload['nonce'] == "$NONCE", f"nonce mismatch: {payload['nonce']}"
assert 'sub' in payload, "no sub claim"
assert payload.get('email') == "$TEST_USER_EMAIL", f"email claim wrong: {payload.get('email')}"
assert 'slatehub_org' in payload, "no slatehub_org claim"
assert payload.get('slatehub_org_role') == 'owner', f"role wrong: {payload.get('slatehub_org_role')}"
print(f"  payload OK: sub={payload['sub']}, role={payload['slatehub_org_role']}")
PY
ok "id_token signature + claims verified"

# ---------- 10. /userinfo --------------------------------------------------
step "GET /userinfo with bearer"
USERINFO=$(curl -sf -H "Authorization: Bearer $ACCESS_TOKEN" "$ISSUER/userinfo")
SUB=$(echo "$USERINFO" | jq -r '.sub')
EMAIL=$(echo "$USERINFO" | jq -r '.email')
[ -n "$SUB" ] && [ "$SUB" != "null" ] || fail "userinfo missing sub"
[ "$EMAIL" = "$TEST_USER_EMAIL" ] || fail "userinfo email mismatch: $EMAIL"
ok "userinfo OK (sub=$SUB, email=$EMAIL)"

# ---------- 11. /token refresh_token grant --------------------------------
# Re-run the full flow asking for offline_access so we get a refresh_token.
step "Refresh-token grant"
PKCE2=$(python3 - <<'PY'
import base64, hashlib, secrets
v = base64.urlsafe_b64encode(secrets.token_bytes(32)).rstrip(b'=').decode()
c = base64.urlsafe_b64encode(hashlib.sha256(v.encode()).digest()).rstrip(b'=').decode()
print(f"{v} {c}")
PY
)
PV2=${PKCE2% *}; PC2=${PKCE2#* }
AURL2="$ISSUER/authorize?response_type=code&client_id=$CLIENT_ID"
AURL2+="&redirect_uri=$(python3 -c "import urllib.parse;print(urllib.parse.quote('$TEST_REDIRECT', safe=''))")"
AURL2+="&scope=openid+offline_access&code_challenge=$PC2&code_challenge_method=S256&state=s2&nonce=n2"
# Consent already granted for openid; offline_access is new → consent again.
HTML2=$(curl -sf -b "$COOKIE_JAR" -c "$COOKIE_JAR" "$AURL2")
if echo "$HTML2" | grep -q 'name="params_json"'; then
  PJ2=$(echo "$HTML2" | grep -oE 'name="params_json" value="[^"]*"' \
        | sed -E 's/name="params_json" value="(.*)"/\1/' \
        | python3 -c 'import sys,html;print(html.unescape(sys.stdin.read()))')
  CB2=$(curl -s -o /dev/null -w '%{redirect_url}' \
    -b "$COOKIE_JAR" -c "$COOKIE_JAR" -X POST "$ISSUER/authorize/consent" \
    --data-urlencode "params_json=$PJ2" --data-urlencode "action=approve")
else
  # consent skipped → 302 directly
  CB2=$(curl -s -o /dev/null -w '%{redirect_url}' \
    -b "$COOKIE_JAR" -c "$COOKIE_JAR" "$AURL2")
fi
CODE2=$(python3 -c "import urllib.parse;print(dict(urllib.parse.parse_qsl(urllib.parse.urlparse('$CB2').query)).get('code',''))")
TR2=$(curl -sf -X POST "$ISSUER/token" -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "grant_type=authorization_code" \
  --data-urlencode "code=$CODE2" \
  --data-urlencode "redirect_uri=$TEST_REDIRECT" \
  --data-urlencode "code_verifier=$PV2")
RT=$(echo "$TR2" | jq -r '.refresh_token')
[ "$RT" != "null" ] && [ -n "$RT" ] || fail "no refresh_token (offline_access scope?)"
ok "got refresh_token"

REFRESHED=$(curl -sf -X POST "$ISSUER/token" -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "grant_type=refresh_token" \
  --data-urlencode "refresh_token=$RT")
NEW_AT=$(echo "$REFRESHED" | jq -r '.access_token')
NEW_RT=$(echo "$REFRESHED" | jq -r '.refresh_token')
[ "$NEW_AT" != "null" ] || fail "refresh did not return new access_token"
[ "$NEW_RT" != "null" ] && [ "$NEW_RT" != "$RT" ] || fail "refresh token did not rotate"
ok "refresh issued new tokens (rotation confirmed)"

# Reuse-detection: replaying old RT must die.
REPLAY=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$ISSUER/token" \
  -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "grant_type=refresh_token" --data-urlencode "refresh_token=$RT")
[ "$REPLAY" = "400" ] || fail "old refresh_token replay should 400, got $REPLAY"
ok "old refresh_token replay correctly rejected"

# ---------- 12. /introspect + /revoke ------------------------------------
step "/introspect"
INTRO=$(curl -sf -X POST "$ISSUER/introspect" -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "token=$NEW_AT")
[ "$(echo "$INTRO" | jq -r '.active')" = "true" ] \
  || fail "introspect should report active=true, got: $INTRO"
ok "introspect: active=true"

step "/revoke (refresh token)"
curl -sf -X POST "$ISSUER/revoke" -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "token=$NEW_RT" >/dev/null
INTRO2=$(curl -sf -X POST "$ISSUER/introspect" -u "$CLIENT_ID:$CLIENT_SECRET" \
  --data-urlencode "token=$NEW_AT")
[ "$(echo "$INTRO2" | jq -r '.active')" = "false" ] \
  || fail "after revoking refresh, access_token should be inactive (chain revoke), got: $INTRO2"
ok "session chain revoked"

printf "\n%s✅ All OIDC checks passed.%s\n" "$GREEN" "$NC"
