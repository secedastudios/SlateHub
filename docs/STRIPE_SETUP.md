# Stripe Setup — Paid Identity Verification

This is the operator-side checklist for getting the `/get-verified` paid flow live. The code is already in place; this is the dashboard + env config you (or the deploy operator) need to do once per environment.

## 1. Stripe account

If you don't already have one: sign up at <https://stripe.com>. Complete business activation (German company details, IBAN for payouts). Until activated you can use test mode for development.

## 2. Enable Stripe Tax

Stripe Dashboard → **Tax** → **Get started**.

- Confirm your origin address (Germany).
- Add your **German VAT ID** under Tax Settings → Tax registrations.
- Enable additional EU registrations as you cross thresholds. For OSS (One-Stop-Shop), register once with the Bundeszentralamt für Steuern; Stripe Tax will then collect the right VAT for every EU country and your quarterly OSS filing covers all of them.
- For non-EU countries (UK, AU, CH, etc.), add the registration in Stripe Tax once you exceed each country's threshold.

Cost: ~0.4% per transaction in monitored locations.

## 3. Enable Stripe Identity

Dashboard → **Identity** → **Get started**. Activate it for your account. Pricing: ~$1.50 per verification check (verify on Stripe's pricing page; rates may change).

## 4. Webhook endpoint

Dashboard → **Developers** → **Webhooks** → **Add endpoint**.

- **Endpoint URL:** `https://<your-domain>/webhooks/stripe`
- **API version:** latest
- **Events to send:**
  - `checkout.session.completed`
  - `identity.verification_session.verified`
  - `identity.verification_session.requires_input`
  - `identity.verification_session.canceled`

After creating, **reveal the signing secret** and copy it — that's `STRIPE_WEBHOOK_SECRET`.

For local development, use the Stripe CLI:
```
stripe listen --forward-to localhost:3000/webhooks/stripe
```
which prints a `whsec_…` secret you can drop into `.env`.

## 5. API keys

Dashboard → **Developers** → **API keys**.

- Copy **Secret key** (`sk_test_…` for test mode, `sk_live_…` for prod) → `STRIPE_SECRET_KEY`
- Copy **Publishable key** (`pk_test_…` / `pk_live_…`) → `STRIPE_PUBLISHABLE_KEY` (we don't currently use this in the server, but keep it in env for any future client-side bits)

## 6. Environment

Add to your `.env`:

```
STRIPE_SECRET_KEY=sk_test_...
STRIPE_PUBLISHABLE_KEY=pk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...
APP_URL=http://localhost:3000   # already used elsewhere; Stripe redirects use it
```

When all three Stripe vars are missing, the paid flow is silently disabled and `/get-verified` falls back to the existing manual-request CTA.

## 7. Boot log check

When the server starts you should see:

```
stripe: configured
```

If you see `stripe: env incomplete — paid verification disabled`, one of the env vars is missing.

## 8. End-to-end test (test mode)

1. Open `/get-verified` while logged in as a user with `verification_status != "identity"`.
2. Click "Get Verified — $10.00" (or whatever your locale shows).
3. On Stripe Checkout, use test card `4242 4242 4242 4242`, any future expiry, any CVC, any postal code.
4. Stripe redirects you to `/get-verified/return`, which immediately creates the Identity session and bounces you to Stripe's hosted ID flow.
5. In test mode, Stripe Identity lets you "simulate success" with any uploaded image. Use this.
6. Webhook fires. Your DB:
   - `verification_payment.status` → `verified`
   - `person.verification_status` → `identity`
7. User lands on `/get-verified/done` with the "You're verified!" page.

## 9. What happens on failure

- User cancels in Stripe Identity → webhook fires `identity.verification_session.canceled` → server auto-refunds the original payment_intent and marks `verification_payment.status = 'refunded'`.
- User pays but never completes ID (closes tab, ignores email) → after 24h the daily background job (`refund_stale_payments`) refunds and marks `refunded`.
- User fails with `requires_input` (bad photo, mismatched selfie) → webhook records `failure_reason`, user can retry within the same Identity session. We do NOT auto-refund here; if they never come back, the 24h job catches it.

## 10. Pricing localization

Prices are hardcoded in `server/src/services/stripe.rs` (`PRICE_TABLE`). To adjust:

```rust
Price { currency: "eur", amount_minor: 1000, label: "€10.00" },
```

`amount_minor` is in the smallest unit of the currency (cents for EUR/USD/GBP, no decimal for JPY). The customer's currency is detected from their `Accept-Language` header; bare-language EU codes (`de`, `fr`, `it`, etc.) all map to EUR, country-tagged ones (`en-GB`, `en-CA`, `en-AU`, `ja`) get their own currency, everything else falls through to USD.

Add a new currency by appending a row to the table and (optionally) extending `pick_price` to map a locale to it.

## 11. Going live

When switching to production keys (`sk_live_`, `pk_live_`, new webhook endpoint):

1. Create a **new** webhook endpoint at the same URL with the same events; copy the live `whsec_…`.
2. Update `STRIPE_SECRET_KEY`, `STRIPE_PUBLISHABLE_KEY`, `STRIPE_WEBHOOK_SECRET` in production env.
3. Restart server.
4. Run one real test transaction (you can refund yourself afterwards).
