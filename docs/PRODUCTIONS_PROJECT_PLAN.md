# SlateHub Productions Management — Project Plan

> Source spec: [`SlateHub_Productions_Product_Specification.md`](../SlateHub_Productions_Product_Specification.md)
> Companion service: [`../aristotle`](../../aristotle) — script-breakdown engine (Rust/Axum/SurrealDB)
>
> **Status legend:** ⬜ not started · 🟡 in progress · ✅ done · ❌ deferred / cut

---

## 1. Executive summary

SlateHub already has a strong **production record** (TMDB-aware, slug-routed, semantic-search-indexed) with member/role graphs (`member_of`, `involvement`), versioned scripts (`production_script`), notifications, messaging, S3, and a feature-flag system. What's missing is the **management workspace** on top: scheduling, call sheets, scene/breakdown data, episodic hierarchy, and the workflow glue that turns a static record into an active project.

This plan brings production-management to MVP in **five phases**, each shippable on its own, with the first phase landing the **highest-priority user-facing features**: management-mode toggle, script versioning UI, aristotle-powered breakdown with manual review, script sharing, scheduling, and call sheets.

**Everything in this plan ships behind a feature flag, locked to slatehub-wide admins (`person.is_admin = true`) at launch.** Promotion to verified users and then to everyone happens against explicit criteria defined in §4.6. Until a slatehub admin flips the `production_management` flag past `admin_only`, no non-admin user sees any of this work — they continue to see the existing public production pages exactly as before.

### 1.1 Priority order (from product owner)

1. ✦ Clean UX for switching between **Overview** and **Management** views of a production
2. ✦ Script upload + version tracking (DB already supports it; UI needs work)
3. ✦ One-off productions **and** series/episodic productions
4. ✦ Script breakdown with simple manual review (aristotle integration)
5. ✦ Script sharing with cast & crew + notifications on update
6. ✦ Scheduling
7. ✦ Call sheets — **high-quality PDF rendering is a P0 capability** (see §2.7); email delivery now, WhatsApp later
8. ✦ Everything else in the spec

---

## 2. Architecture decisions

### 2.1 Aristotle integration — **in-process Rust crate**

Aristotle becomes a **pure processing library** that lives in this repo as a Cargo workspace member. No HTTP server, no job queue, no web UI, no own database — those are stripped. Slatehub calls aristotle's functions directly from its own async runtime; the existing slatehub backend owns all UX, routing, storage, and orchestration.

**Repo layout (after conversion):**
```
slatehub/                       ← workspace root
├── Cargo.toml                  ← workspace
├── server/                     ← slatehub axum server
│   └── Cargo.toml              ← depends on `aristotle = { path = "../aristotle" }`
├── aristotle/                  ← library crate
│   ├── Cargo.toml              ← lib only, no axum / no server deps
│   ├── src/
│   │   ├── lib.rs              ← public API
│   │   ├── parser/             ← PDF, FDX, Fountain, Fade In
│   │   ├── breakdown/          ← Tier 0–2 + optional Tier 4 LLM
│   │   ├── dedupe/             ← optional cross-scene dedupe
│   │   ├── schedule/           ← bin-packing shoot schedule
│   │   └── models.rs           ← Scene, Breakdown, etc.
│   └── tests/                  ← keep the parser + breakdown tests
├── docker-compose.yml
└── docs/
```

**What goes — dropped from aristotle:**
- `src/main.rs`, `src/handlers.rs` — HTTP server
- `src/queue.rs` — job queue (slatehub spawns its own `tokio::task::spawn`)
- Aristotle's `Dockerfile`, `docker-compose.yml`, web UI templates
- Aristotle's own SurrealDB schema + storage layer — results are returned to the caller as structs, slatehub writes them into its own `scene` + `breakdown_item` tables
- `axum`, web-framework deps from `Cargo.toml`

**What stays — what we want from aristotle:**
- All 4 parsers (PDF, FDX, Fountain, Fade In)
- The deterministic Tier 0–2 breakdown pipeline (slug-line regex, character cues, heuristic dictionaries)
- Optional Tier 4 LLM classifier (Ollama / Anthropic client)
- Cross-scene dedupe (off by default)
- The shooting-schedule bin-packer
- The 23 unit/integration tests that exercise the above

**Public API (sketch — refine during conversion):**
```rust
// aristotle/src/lib.rs
pub use crate::models::{Script, Breakdown, SceneBreakdown, BreakdownItem, ShootSchedule, ...};

#[derive(Debug, Clone, Copy)]
pub enum ScriptFormat { Pdf, Fdx, Fountain, FadeIn }

#[derive(Debug, Default)]
pub struct BreakdownOpts {
    pub policy: BreakdownPolicy,         // Deterministic | Hybrid (LLM on)
    pub llm_backend: Option<LlmBackend>, // Ollama { url } | Anthropic { api_key }
    pub dedupe: bool,
}

/// Synchronous — pure parsing. Returns the parsed script structure.
pub fn parse(input: &[u8], format: ScriptFormat) -> Result<Script>;

/// Async — Tier 4 LLM calls hit HTTP. Tier 0–2 are CPU-bound but cheap.
/// Heavy work happens here; callers should spawn this on a `tokio::task`.
pub async fn breakdown(script: &Script, opts: &BreakdownOpts) -> Result<Breakdown>;

/// Synchronous — pure bin-packing on top of a Breakdown.
pub fn schedule(breakdown: &Breakdown, opts: &ScheduleOpts) -> Result<ShootSchedule>;
```

**Async pattern in slatehub:**
```rust
// In a slatehub route handler (illustrative):
let script_bytes = s3.download_file(&script.file_key).await?;
let aristotle_job_id = create_aristotle_job_row(&production, &script).await?;

// Fire-and-forget; results land in DB via the spawned task.
tokio::task::spawn(async move {
    let parsed = match aristotle::parse(&script_bytes, ScriptFormat::Pdf) {
        Ok(s) => s,
        Err(e) => return mark_job_failed(aristotle_job_id, e).await,
    };
    match aristotle::breakdown(&parsed, &BreakdownOpts::default()).await {
        Ok(b) => persist_breakdown_to_db(aristotle_job_id, b).await,
        Err(e) => mark_job_failed(aristotle_job_id, e).await,
    }
});

// Handler returns immediately with "breakdown queued" — Datastar SSE on the
// page polls/streams the aristotle_job row until it flips to `complete`.
Ok(Redirect::to(&breakdown_review_url))
```

**Why this is the right call (vs. sidecar):**
- No HTTP overhead, no JSON marshaling, no webhook signature dance
- Direct Rust API with full type safety — refactors are atomic, compiler-checked
- One binary, one deploy, one set of env vars
- Slatehub already has the spawn-tokio-task pattern (embedding generation, listmonk, stale-payment refunds) — breakdown fits the same model
- Aristotle's tests live with the code they test; they pass when slatehub passes

**What we're trading off:**
- Slatehub binary gets heavier (PDF + FDX + LLM deps come along). Acceptable.
- Can't scale breakdown CPU independently of slatehub HTTP load. Mitigation: spawn breakdown tasks with a semaphore to cap concurrency; horizontal-scale slatehub if it becomes a bottleneck.
- Lose aristotle's standalone debugging UI. Mitigation: slatehub builds an admin view at `/admin/breakdowns` showing recent `aristotle_job` rows with raw/parsed output (Phase 1.4).

**Aristotle config flows through `BreakdownOpts`** — slatehub reads env vars at startup (`ARISTOTLE_POLICY=deterministic|hybrid`, `ARISTOTLE_LLM_BACKEND=...`, `ARISTOTLE_OLLAMA_URL=...`, `ANTHROPIC_API_KEY=...`) and assembles a `BreakdownOpts` struct, optionally overridden per-production via feature flag or admin setting.

### 2.2 Episode / season / series model

Three formats need first-class support:
- **One-off** (feature, short, music video, vertical) — current `production` model works as-is.
- **Series without seasons** (YouTube series, podcast feed, vertical drama with one continuous arc) — flat list of episodes.
- **Series with seasons** (traditional TV) — episodes grouped under seasons.

The model handles both with a nullable season link, so the schema doesn't fork:

```
season {
    id, production, season_number, title, description,
    premiere_date, finale_date, status, created_at
}

episode {
    id,
    production,                  -- always set
    season,                      -- nullable: NULL = "series without seasons"
    episode_number,
    title, slug, description, status,
    air_date, created_at, updated_at
}
```

**Unique index:** `(production, season, episode_number)` UNIQUE. With nullable `season` this still works correctly in SurrealDB v3 — `(prod:x, NULL, 1)` and `(prod:x, NULL, 1)` collide as expected, so flat series get correct enforcement.

**UI behavior — no upfront question.** Don't make the user declare "does this series have seasons" at creation time. Instead:
- New series defaults to flat list; "Add episode" creates an episode with `season = NULL`.
- "Group into seasons" action on the episodes page creates a `season` row and bulk-assigns existing episodes to it.
- A series can mix: legacy flat episodes (`season = NULL`) coexist with season-grouped episodes. The UI shows the flat ones in an "Unassigned" group.

**URL shape:** `/productions/{slug}/episodes/{ep_slug}` for both modes — season is metadata, not a URL component. Saves us from URL changes when a series promotes from flat to seasoned.

The existing `production_script.production` foreign key gets a sibling field `episode` (option<RecordId<episode>>) so scripts can attach to either the production directly (films, series bibles, format docs) or a specific episode.

Scheduling, breakdown, and call sheets work at whichever level fits — episode for series, production for one-offs. UI dispatches based on `production.type`.

### 2.3 Management Mode UX

A toggle in the production header lets editors flip between:

- **Overview** (current view) — public-facing summary, credits, photos, scripts (per visibility).
- **Management** — workspace tabs: Script · Breakdown · Schedule · Call Sheets · Team · Files · Budget.

Persisted in URL: `/productions/{slug}` vs `/productions/{slug}/manage/...`. No new auth model — same `member_of` permission applies (`owner` / `admin` / `member` see management; non-members redirected to overview).

### 2.4 Permissions — two layers

Every management-mode access check happens through **two gates** in sequence. Both must pass.

**Layer 1 — global feature flag (`production_management`).**
`feature_flag::allows("production_management", Some(&user))` decides whether *anyone* can see management mode at all. Initially set to `admin_only`, meaning only slatehub-wide admins (`person.is_admin = true`) clear this layer. We flip it to `verified` (then `all`) as the feature matures.

**Layer 2 — per-production membership (`member_of.role`).**
Even after clearing Layer 1, the user must be a `member_of` the specific production they're trying to manage, with role `owner` / `admin` / `member`. A slatehub admin who isn't a member of production X still cannot manage X.

In code, every management route does:

```rust
async fn manage_handler(
    AuthenticatedUser(user): AuthenticatedUser,
    Path(slug): Path<String>,
) -> Result<...> {
    if !feature_flag::allows("production_management", Some(&user)).await {
        return Err(Error::NotFound);  // 404, not 403 — feature doesn't exist
    }
    let prod = require_production_member(&user, &slug, MinRole::Member).await?;
    // ...
}
```

Returning `NotFound` (rather than `Forbidden`) when the flag is off is deliberate — non-admins shouldn't be able to enumerate what management features exist.

**Sub-feature flags layered on top** for finer control:
- `script_breakdown` — gates the aristotle breakdown trigger + review UI
- `call_sheet_email` — gates the "Publish & Email" call sheet action
- (More added per phase.)

These let us turn off a single subsystem without rolling back the whole management feature.

**Role-level granularity inside management:**

| Role | Default capability |
|---|---|
| `owner` | Everything, including delete & change ownership |
| `admin` | Everything except destructive ops & ownership change |
| `member` | Read + comment + own-section edits (e.g. confirm their own call time) |

The existing `member_of.permissions: array<string>` field is reserved for v2 fine-grained ACLs (`script.upload`, `schedule.edit`, etc.). For v1 the role tier is sufficient.

### 2.5 Notifications

Reuse `NotificationModel::create()`. Add new `notification_type` values:
- `script_uploaded` — new script version available
- `breakdown_ready` — aristotle finished
- `schedule_changed` — shoot day edited
- `call_sheet_sent` — daily call sheet published
- `production_role_assigned` — added to crew

Each notification has `related_id` = production slug or episode id for deep-linking.

### 2.6 Delivery channels for call sheets

- **Phase 2:** email via existing `EmailService` (Mailjet).
- **Phase 4:** WhatsApp via WhatsApp Business API (existing `whatsapp-bot/` directory in the repo — investigate state of that).
- **Always:** in-app notification + downloadable PDF.

### 2.7 PDF call sheet rendering — high priority

**Producers judge production tools on call sheet quality.** A messy PDF undermines trust in everything else. We treat PDF rendering as a P0 capability, not a Phase 2 spike.

**Industry-standard call sheet must include**, formatted to fit on a single landscape A4/Letter page (or two for large crews):

- Header: production name, episode (if series), shoot day # of total, date, day of week
- Production company logo + contact
- Director / 1st AD / 2nd AD / UPM contact rows
- General crew call + breakfast/lunch times
- Weather forecast + sunrise/sunset (hooked to a weather API later — leave a placeholder for now)
- Scene table: scene #, page count, INT/EXT, location, cast in scene, est. duration
- Cast section: each cast member with character, call time, hair/makeup time, set call, status (W = working, H = hold, SW = start work, etc.)
- Crew section: grouped by department with name, role, call time, wrap (optional)
- Locations: each location with full address, parking notes, base camp, lunch location
- Emergency: nearest hospital with address, on-set medic info
- Distribution: who received the sheet
- Notes / safety reminders

Strong reference is StudioBinder's output — that's the bar.

**Rendering technology — decide in Phase 0:**

| Option | Quality | Complexity | Recommend? |
|---|---|---|---|
| `printpdf` (Rust, already in aristotle) | Low–medium (manual layout, no flow) | Low | ✗ Not for call sheets — too low-level for a complex grid |
| `genpdf` (built on printpdf) | Medium (paragraph flow, basic tables) | Medium | △ Maybe — limited table styling |
| `typst` invoked as subprocess | **High** (typesetting-grade output) | Medium (template files + binary) | ✓ Strong candidate |
| Headless Chromium (via `chromiumoxide` or `wkhtmltopdf`) | **High** (full HTML/CSS) | High (browser dep + sandboxing) | ✓ Strong candidate for "matches the on-screen preview exactly" |
| Server-side PDF lib in another language (Puppeteer microservice, WeasyPrint sidecar) | High | High (another service) | △ Only if Rust options fall short |

**My recommendation: Typst.** It's a modern typesetting system designed for exactly this kind of document, produces PDF output that rivals LaTeX without LaTeX's complexity, can be invoked as a binary subprocess from Rust, and the template language is far easier than manual `printpdf` layout. Call sheet template lives as a `.typ` file under `server/templates/call_sheets/` — Rust generates the data context, renders via subprocess, returns PDF bytes.

**Phase 0 spike:** build a one-page call sheet PDF with both Typst and headless Chromium, A/B them, pick the winner. Document the choice + a sample PDF in `docs/CALL_SHEET_PDF.md`.

**A note on iteration:** the on-screen HTML preview and the PDF must use the **same data model** so producers can WYSIWYG-edit and trust what prints. Strategy: render the HTML preview from the same context object that gets serialized into Typst variables (or piped into headless Chromium if we go that route).

---

## 2.X Engineering conventions (apply to every task)

These three skill profiles govern how every UI and backend task below is built. They're non-negotiable; if a PR violates them it gets sent back. Cross-referenced for any contributor unfamiliar with them.

### `rust-saas` — backend shape
- **Stack:** Rust + Axum + SurrealDB v3 + Askama server-rendered templates + Datastar SSE for live updates. Same as the rest of slatehub.
- **No SPA.** No React/Vue/Svelte. Hypermedia drives the UI.
- **Auth via JWT cookies** (existing pattern); session loads through the existing `auth_middleware`.
- **Models** under `server/src/models/`; **services** under `server/src/services/`; **routes** under `server/src/routes/`; **templates** under `server/templates/`.
- **DB:** singleton `LazyLock<Surreal<Client>>`. Bind `RecordId` directly as query params — never hand-parse `"table:key"` strings (see [`feedback-record-id-handling`](../../../.claude/projects/-Users-chris-src-slatehub/memory/feedback_record_id_handling.md)).
- **Errors:** project-wide `crate::error::Error` enum; never return raw strings or stringly-typed Stripe-style errors.
- **Feature flags** gate every Phase 1+ feature behind a `services/feature_flag.rs` entry — default `admin_only` per §4.6.

### Tests — **hard rule: integration tests under `server/tests/`, never inline `#[cfg(test)] mod tests`**

Why: discoverability (`ls tests/`), forcing testing through the public API, single source of truth for "is this module tested" (existence of `tests/<feature>_test.rs`).

- **Every new model, service, route file, or migration ships with a test file** at `server/tests/<feature>_test.rs`. No exceptions. If a feature is too tangled to test from the outside, refactor until it isn't.
- **No `#[cfg(test)] mod tests { ... }` blocks at the bottom of source files.** Existing ones get migrated to `tests/` when the surrounding code is touched.
- **Layout** mirrors the source: `models/season.rs` → `tests/season_test.rs`, `services/aristotle_runner.rs` → `tests/aristotle_runner_test.rs`, `routes/episodes.rs` → `tests/episode_routes_test.rs`.
- **Shared fixtures** live in `tests/common/mod.rs` (existing pattern). Add helpers there when more than one test file needs them.
- **Pure-logic tests** (no DB, no IO) still go in `tests/` — they're cheap to run and we maintain one consistent location. See `tests/stripe_webhook_test.rs` for the established pattern.
- **What "ample" means:**
  - At least one happy-path integration test per public function / route
  - Each documented failure mode (validation error, permission denied, missing row, race) covered by an assertion
  - For state machines (e.g. `verification_payment.status` transitions, `aristotle_job.status` transitions), exercise every legal transition + at least one illegal-transition guard
  - Idempotency assertions where webhooks or retries are involved (Stripe webhook deliveries, breakdown re-runs)
  - Cascade tests extended in `tests/cascade_delete_test.rs` for every new person-linked table (see [`feedback-cleanup-protection`](../../../.claude/projects/-Users-chris-src-slatehub/memory/feedback_cleanup_protection.md))
- **Public visibility may need a bump.** Tests under `tests/` see only `pub` items from `slatehub::*`. If a private helper genuinely can't be exercised through the public surface, prefer a `pub(crate)`-equivalent design change (re-export via a thin facade) over making the test inline. **Do not** add `#[cfg(test)] mod tests` as the escape hatch.
- **Per-task checkbox.** Each implementation cluster in Phase 1+ ends with an explicit `Tests at server/tests/<file>_test.rs:` checkbox. Don't tick the cluster as complete until those tests exist and pass.

### `html-purist` — markup discipline
- **Zero inline styles.** No `style="…"` attributes. Ever. All styling lives in CSS files under `server/static/css/`.
- **Semantic HTML.** Use `<section>`, `<article>`, `<nav>`, `<header>`, `<dialog>`, `<table>`, `<form>` — not `<div>` soup. ARIA only where native semantics fall short.
- **CSS custom properties** for the design system (already established under `static/css/main.css` `:root`). Reuse the existing token names (`--color-*`, `--space-*`, `--text-*`).
- **CSS-first animations and transitions.** No JS-driven animation libraries.
- **Minimal JavaScript.** Datastar attributes (`data-on-click`, `data-bind`, `data-signals`) drive reactivity. Server sends SSE patches to update fragments. We do not write hand-rolled fetch + DOM manipulation.
- **JSON-LD** in `<script type="application/ld+json">` for SEO when relevant (production pages benefit; admin pages don't need it).
- **Forms post to the server** and re-render on the back-end. No client-side validation that isn't also enforced server-side.

### `frontend-design` — visual quality
- **Distinctive, polished UI.** Match the existing slatehub aesthetic (dark theme, `#171717` background, `#d6d8ca` text, `#eb5437` accent, geometric sans typography). Don't introduce a parallel design system.
- **Spacing on the 4px grid.** Reuse the existing `--space-*` token scale.
- **Typography hierarchy.** Use the existing heading/body styles in `main.css`; don't roll new sizes.
- **States visible to the user.** Every interactive surface has hover / focus / active / disabled / error states. Empty states have illustrations or at least clear copy + an action.
- **Responsive by default.** Single-column on mobile, grid/multi-column on desktop. Touch targets ≥ 44px. Especially critical for on-set call sheet view (§4.x).
- **High-density admin views are OK** (the existing admin pages prove this) — small text + compact rows are acceptable when the audience is power users.
- **Polish loaded states.** Optimistic SSE updates rather than spinners where possible.

### Cross-cutting application examples

| Task type | rust-saas tells me | html-purist tells me | frontend-design tells me |
|---|---|---|---|
| New management tab | Axum route handler + Askama template | Semantic sections, CSS file under `static/css/pages/manage/` | Match existing nav patterns + responsive grid |
| New form (e.g. season create) | POST + Form extractor + server-side validation | Real `<form>` with `<label>` for every field, no inline styles | Polished input states (focus rings via token), clear error placement |
| Live update (e.g. breakdown progress) | Datastar SSE endpoint sending patch elements | `<div data-signals-…>` + SSE-targeted ids | Progress affordance — never a bare spinner |
| Notification badge | Existing `NotificationModel`; SSE on count change | `<output>` element with `aria-live="polite"` | Number animates in via CSS, not JS |

### Action: invoke the skills at the start of every implementation pass

Whenever a Phase task moves to 🟡 in_progress, invoke the relevant skill profile via the Skill tool (`rust-saas`, `html-purist`, `frontend-design`) so the full skill guidance is loaded into context for that pass. This avoids drift over long sessions.

---

## 3. Phased rollout

Each phase is **independently shippable**, behind a feature flag (`production_management` registered in `services/feature_flag.rs`). Default state per phase noted below.

---

## Phase 0 — Foundation (weeks 1)

**Goal:** put the building blocks in place so the rest of the work can run in parallel.

### 0.1 Schema additions ✅
- [x] ✅ **Migration: season + episode tables** — `season` (unique `(production, season_number)`) and `episode` (unique `(season, episode_number)` + unique `(production, slug)`). Per the SurrealDB v3 NULL-uniqueness gotcha caught by the test, `episode.season` is required; flat series auto-create a "Season 1" at the application layer. File: `db/migrations/012_seasons_episodes.surql` + mirrored into `schema.surql`.
- [x] ✅ **Migration: scene + breakdown_item tables** — `scene` (production/episode/script links, slug-line metadata, status) + `breakdown_item` (category enum matching aristotle's output, confidence, source: auto/manual/edited). File: `db/migrations/013_scenes_breakdown.surql`.
- [x] ✅ **Migration: schedule + call_sheet tables** — `schedule_day` + `schedule_scene` + `call_time` (unique `(day, person)`) + `call_sheet` (versioned per day). File: `db/migrations/014_scheduling.surql`.
- [x] ✅ **Migration: aristotle_job table** — tracks in-process breakdown runs; no external job id. File: `db/migrations/015_aristotle_jobs.surql`.
- [x] ✅ **Migration: add `episode` field to `production_script`** — DEFINE FIELD OVERWRITE, idempotent. File: `db/migrations/016_production_script_episode.surql`.
- [x] ✅ **Update `Person::delete_with_cascade`** — deletes `call_time WHERE person`, nulls `call_sheet.generated_by` + `sent_by`. Other new tables are production-owned (no person ref).
- [x] ✅ **Tests at `server/tests/cascade_delete_test.rs`** — extended to seed schedule_day + call_time + call_sheet (with chris as `generated_by` and `sent_by`) and assert deletion / null-out behavior. Passing.
- [x] ✅ **Tests at `server/tests/schema_migrations_test.rs`** — 12 constraint tests covering every ASSERT and UNIQUE index in the new tables. Especially exercises the flagship case `episode_number_one_can_repeat_across_different_seasons` and `episode_requires_a_season`. Passing.

### 0.2 Feature flag — **registered as `admin_only` from day one** ✅

So that the developer (a slatehub admin) can verify each task as it lands without flipping anything manually after every deploy.

- [x] ✅ Extended `FlagDef` in `services/feature_flag.rs` with `initial_state: FlagState`. Existing `identity_verification` flag declares `initial_state: FlagState::Off`; the three new flags declare `AdminOnly`.
- [x] ✅ Updated `register_flags()` to use `initial_state` when seeding a missing row. Existing rows preserved (operator-set state survives reboot).
- [x] ✅ Registered `production_management` flag with `initial_state: FlagState::AdminOnly`.
- [x] ✅ Registered `script_breakdown` flag with `initial_state: FlagState::AdminOnly`.
- [x] ✅ Registered `call_sheet_email` flag with `initial_state: FlagState::AdminOnly`.
- [x] ✅ Tests at `server/tests/feature_flag_test.rs` — `test_register_flags_seeds_initial_state_for_new_flags` + `test_register_flags_preserves_operator_state_on_existing_row`. Passing.

### 0.3 PDF call sheet rendering spike (HIGH PRIORITY)

Don't wait until Phase 2 — quality of the call sheet PDF is the make-or-break feature for producers. Resolve the rendering technology *before* we build the schedule UI, because it shapes the data model (HTML preview must match PDF, see §2.7).

- [ ] ⬜ **Build a static reference call sheet** in Typst — populate with sample data covering every section in §2.7 (header, scene table, cast/crew, locations, emergency). Iterate until it looks like a polished StudioBinder-style output.
- [ ] ⬜ **Build the same call sheet** via headless Chromium with an HTML template + CSS print rules.
- [ ] ⬜ **Side-by-side comparison:** print both, A/B with at least one producer if possible. Compare on:
  - Single-page fit (no awkward overflow)
  - Table layout (cast/crew columns aligned, no broken rows)
  - Typography (legible at print size, no font-fallback ugliness)
  - File size (target < 200KB for a typical 3-page sheet)
  - Generation latency (target < 1s per sheet)
  - Container image size (don't bloat Docker; headless Chromium adds ~300MB)
- [ ] ⬜ **Pick a winner** and document the decision in `docs/CALL_SHEET_PDF.md` (committed reference sample PDF included).
- [ ] ⬜ **Scaffolding:** `server/src/services/call_sheet_pdf.rs` with a `render(context: CallSheetContext) -> Result<Vec<u8>>` function. Stub it returning placeholder bytes for now — Phase 2 fills in the real implementation but downstream code can integrate against the API immediately.
- [ ] ⬜ **HTML preview template** (`templates/productions/manage/call_sheet_preview.html`) built from the **same context struct** the PDF renderer takes. This is the "what you see is what you print" guarantee.
- [ ] ⬜ **Tests at `server/tests/call_sheet_pdf_test.rs`** — render a known `CallSheetContext`, assert (a) PDF starts with `%PDF-`, (b) non-empty bytes, (c) page count is in expected range (1–3 for typical sheet), (d) re-rendering same context is byte-deterministic (or close — flag if not, decide if we care). Visual quality verified manually + committed reference PDF in `docs/CALL_SHEET_PDF.md`.

**Tentative pick if you want to start before the spike:** Typst. Subprocess invocation from Rust is straightforward, output quality is excellent, and the template is far more maintainable than a CSS print-stylesheet. But: validate against actual producer feedback in the spike.

### 0.4 Aristotle → workspace crate conversion

The work to turn aristotle from a self-contained service into a focused library. Run this in parallel with 0.1–0.3.

#### 0.4.1 Workspace setup ✅
- [x] ✅ Promoted slatehub root to a Cargo workspace with `members = ["server", "aristotle"]` and `[workspace.dependencies]` for shared crates.
- [x] ✅ Server depends on `aristotle = { path = "../aristotle" }`.
- [x] ✅ `cargo check` from root builds both members.
- [x] ✅ Used server's existing lockfile as the workspace lockfile to keep version-pinning intact (avoided an rmcp 1.2 → 1.7 surprise that broke a `non_exhaustive` struct constructor).

#### 0.4.2 Move aristotle source ✅
- [x] ✅ Copied `/Users/chris/src/aristotle/src/` → `/Users/chris/src/slatehub/aristotle/src/`. No history preserved.
- [x] ✅ Tests + Cargo.toml + `examples/` fixtures copied. Original `../aristotle` archived (caller's choice — left in place for reference).

#### 0.4.3 Strip the HTTP / service layer ✅
- [x] ✅ Deleted `main.rs`, `handlers.rs`, `queue.rs`, `db.rs`, web templates, `Dockerfile`, `docker-compose.yml`, `Makefile`, `static/`, `schema.surql`, all prompt files.
- [x] ✅ Dropped `axum`, `askama`, `tower`, `tower-http`, `sysinfo`, `dotenvy`, `tracing-subscriber`, `surrealdb`, `uuid` (where unused) from `aristotle/Cargo.toml`.
- [x] ✅ Aristotle is now lib-only (no `[[bin]]`).
- [x] ✅ Also deleted `src/rag/` (chunk indexing + entity graph — entirely DB-coupled, can be reintroduced if Tier 4 LLM quality demands it), `src/graph.rs` (production graph builder — slatehub builds its own), `src/schedule.rs` (Phase 2 will be slatehub's own), and `src/export/` (Phase 5 feature). Smaller, focused public surface.

#### 0.4.4 Design the public API ✅
- [x] ✅ `aristotle/src/lib.rs` exposes top-level re-exports: `parse_screenplay`, `run_breakdown`, `BreakdownPolicy`, `BreakdownContext`, `ParsedScript`, `ParsedScene`, `ScreenplayElement`, `ElementKind`, `Tag`, `TagSource`, `SceneBreakdown`, `ScriptMetadata`, `Config`, `LlmProvider`, `LlmClient`.
- [x] ✅ Stripped surrealdb `SurrealValue` derives from `models.rs` — types are now serde-only, storage-agnostic. Callers map to their own DB representation at the boundary. `Tag`, `TagSource`, `SceneBreakdown` etc. all kept; DB-record types (`Job`, `ScriptMetadataRecord`, `ScriptElementRecord`, `Chunk`) deleted.

#### 0.4.5 Re-validate the tests ✅
- [x] ✅ `cargo test -p aristotle` — **89 tests pass** (66 unit + 23 integration), 1 ignored. Parser + breakdown round-trip tests all survived.
- [x] ✅ HTTP/queue/DB tests automatically dropped along with their modules.

#### 0.4.6 Wire into slatehub ✅
- [x] ✅ **Service module:** `server/src/services/aristotle_runner.rs` — wraps `aristotle::parse_screenplay` + `aristotle::run_breakdown` with project `Error`, structured logging, semaphore-capped concurrency, `spawn_blocking` for CPU-bound parse.
- [x] ✅ **Concurrency cap:** `LazyLock<Semaphore>` with `MAX_CONCURRENT = 4` permits.
- [x] ✅ **Tests at `server/tests/aristotle_runner_test.rs`** — 5 tests covering happy path (Fountain → 2 scenes), speaking-cast detection, corrupt-bytes error path, unsupported-extension error path, and concurrency (8 jobs through 4 permits, all succeed). Passing.
- [ ] ⬜ Env vars `ARISTOTLE_POLICY`, `ARISTOTLE_LLM_BACKEND`, etc. — **deferred until Phase 1.4** when we actually expose the policy/LLM toggle to handlers. Default `DeterministicOnly` hard-coded for now.

**Exit criteria for Phase 0:** ✅ Migrations applied, cascade extended, schema constraint tests cover every new ASSERT/UNIQUE, feature flags seeded as `admin_only`, aristotle is a workspace crate with `aristotle_runner` callable from slatehub. **Phase 0.3 (PDF spike) is the only outstanding piece** — deferred until we begin Phase 2.

#### 0.4.7 Flatten back to a single crate ✅ (post-Phase-0)
- [x] ✅ Removed the workspace and inlined aristotle as `server/src/aristotle/` (still a self-contained module: every internal ref goes through `crate::aristotle::*`, so it can be extracted back into its own crate later).
- [x] ✅ aristotle's deps (`quick-xml`, `pdf-extract`, `osf`) merged into `server/Cargo.toml`; `printpdf` added to `[dev-dependencies]` for the breakdown integration test.
- [x] ✅ `aristotle/tests/breakdown_integration.rs` → `server/tests/aristotle_breakdown_integration_test.rs`; fixtures live at `server/tests/fixtures/aristotle/`.
- [x] ✅ Deleted workspace root `Cargo.toml` + `Cargo.lock`, deleted `aristotle/` and unrelated `whatsapp-bot/` POC.
- [x] ✅ Single Cargo manifest at `server/Cargo.toml`. No workspace overhead, no cross-CWD cache invalidation, faster `make dev`.

---

## Phase 1 — Management Mode + Script + Breakdown (weeks 2–5)

**Goal:** the highest-priority user features. After Phase 1 a producer can: see two views of their production, upload script versions, kick off automated breakdown, manually review/edit it, and share scripts with cast/crew who get notified.

### 1.1 Management mode UX
- [ ] ⬜ **Route scaffolding:** `/productions/{slug}/manage` → renders the management shell. All sub-routes below are nested.
- [ ] ⬜ **Permission middleware:** require `member_of.role` ∈ {owner, admin, member} to access `/manage/*`
- [ ] ⬜ **Production header:** "View as: [Overview ▾] [Management]" segmented toggle (matches existing nav patterns)
- [ ] ⬜ **Management sidebar nav:** Script · Breakdown · Schedule · Call Sheets · Team · Files (placeholder) · Budget (placeholder)
- [ ] ⬜ **Templates:** `templates/productions/manage/_layout.html`, `script.html`, `breakdown.html`, `schedule.html`, `call_sheets.html`, `team.html`
- [ ] ⬜ **Tests at `server/tests/manage_access_test.rs`** — happy path (owner accesses `/manage`), permission denied (non-member redirected), unauthenticated (redirected to login), flag-off (production_management = `off` returns 404), flag-on but `is_admin = false` (non-admin redirected to public overview). Cover all four combinations.

### 1.2 Episode / season / series support
- [ ] ⬜ **Episode CRUD model:** `models/episode.rs` — `create`, `list_for_production`, `list_for_season`, `get_by_slug`, `update`, `delete`
- [ ] ⬜ **Season CRUD model:** `models/season.rs` — `create`, `list_for_production`, `get`, `update`, `delete`, `assign_episodes(season_id, episode_ids)`
- [ ] ⬜ **Episode routes:** nested under `/productions/{slug}/manage/episodes/`
  - GET / — list episodes for series (grouped by season if seasons exist; flat otherwise)
  - GET/POST `/new` — create episode (optional season picker — defaults to NULL if no seasons yet)
  - GET/POST `/{ep_slug}/edit`
  - POST `/{ep_slug}/delete`
- [ ] ⬜ **Season routes:** nested under `/productions/{slug}/manage/seasons/`
  - GET / — manage seasons
  - GET/POST `/new` — create season
  - POST `/{season_id}/assign` — bulk-assign episodes to this season (used by the "Group into seasons" flow)
  - POST `/{season_id}/delete` — deleting a season un-assigns its episodes back to NULL, doesn't delete them
- [ ] ⬜ **"Group into seasons" upgrade flow:** action on the episodes page when current state is flat. Lets you create Season 1, optionally bulk-assign episodes, switches the UI to grouped mode automatically (it's implicit — no toggle on the production, just the presence of season rows).
- [ ] ⬜ **Conditional UI:** if `production.type` is series-like (TV Series, Vertical Series, etc.), show "Episodes" + "Seasons" sidebar items; otherwise hide both.
- [ ] ⬜ **Series production view:** episode list. If any seasons exist, group by season with collapsible sections; "Unassigned" group holds episodes with `season = NULL`. If no seasons exist, flat numbered list.
- [ ] ⬜ **Episode detail page:** mirrors the production management shell but scoped to one episode (scripts, breakdown, schedule live here for series).
- [ ] ⬜ **Tests at `server/tests/episode_test.rs` + `server/tests/season_test.rs` + `server/tests/episode_routes_test.rs`:**
  - Create flat series (no seasons) → add 3 episodes → all show in flat list
  - Create season → assign episodes → episodes show grouped, unassigned section appears if any remain
  - Delete season → episodes un-assign back to NULL, are not deleted
  - Unique-index check: `(production, season=NULL, episode_number=1)` twice should be rejected
  - Unique-index check: `(production, season=s1, episode_number=1)` and `(production, season=NULL, episode_number=1)` coexist fine
  - Permission denied (non-member tries to add episode)
  - Flag-off (production_management `off` returns 404 on episode routes)

### 1.3 Script versioning UI
- [x] ✅ **Script upload form:** title + PDF picker + visibility + notes, wired to the existing `routes/productions.rs::upload_script` handler. Lives in the management Script tab now; redirects post-upload to `/manage/script`.
- [x] ✅ **Version history list:** `ScriptModel::list_grouped_by_title` returns one group per title with `latest` pulled out and `older` in version DESC order (single query, link-traversed uploader). Rendered as a card per title with a `<details>` disclosure for earlier versions.
- [ ] ⬜ **Side-by-side diff:** for v1, "open both PDFs in tabs" approach. *Deferred — link-style "View" opens any version in a new tab today.*
- [x] ✅ **Per-version actions:** view (new tab), download (`download` attr), toggle visibility, delete — all gated on `can_edit` (owner/admin).
- [x] ✅ **Version notes field:** plumbed end-to-end (form → handler → schema → list view + per-version `older` row).
- [x] ✅ **Latest-version highlight:** "v3 · latest" badge in accent color on the card header; older versions are visually demoted inside the disclosure.
- [x] ✅ **Tests at `server/tests/script_management_test.rs`** — empty case, single upload, multi-version grouping, alphabetical multi-title ordering, uploader link-traversal, auto-version increment per (production, title), per-production isolation of version sequences, visibility round-trip, delete returns file_key, idempotent delete of unknown id, parity with `get_latest_for_production`.

### 1.4 Script breakdown via aristotle crate
- [ ] ⬜ **Trigger:** "Run breakdown" button on script detail (gated by `script_breakdown` flag).
- [ ] ⬜ **Handler:** creates `aristotle_job` row in `queued` state, spawns a `tokio::task` through the `aristotle_runner` semaphore (from Phase 0.4.6) so we never overload the box, returns immediately. Datastar SSE on the page streams the job status.
- [ ] ⬜ **Worker task** (running inside the request-spawned `tokio::task`):
  - Mark job `running` + `started_at`
  - Download script from S3 (cheap, async)
  - `aristotle::parse(bytes, format)` (CPU-bound, sync inside `spawn_blocking` if it becomes hot)
  - `aristotle::breakdown(&parsed, &opts).await` (async — Tier 4 LLM calls are HTTP)
  - Persist result via a single multi-statement SurrealQL `BEGIN TRANSACTION` writing `scene` + `breakdown_item` rows
  - Mark job `complete` + `completed_at`
  - On panic / error: mark `failed` + record `error` (use `tokio::task::catch_unwind` or a result wrapper)
- [ ] ⬜ **Notification:** `breakdown_ready` → script uploader + production admins, only on `complete`.
- [ ] ⬜ **Breakdown review UI:** `/manage/scripts/{script_id}/breakdown`
  - Scene-by-scene list, each scene shows: heading, location, int/ext, time-of-day, page count, then chips for cast, props, wardrobe, etc. by category.
  - Each chip: confidence indicator + remove button + edit-in-place (Datastar bind).
  - "Add" button per category to add missed items.
  - "Approve scene" button per scene → flips `scene.status` to `reviewed`.
  - All edits POST back to the server; no client-only state (consistent with `html-purist`).
- [ ] ⬜ **Re-run button:** if the user wants a fresh breakdown after editing the source script — creates a new `aristotle_job` row, keeps the old one for audit. Old `scene` + `breakdown_item` rows linked to the previous run get superseded (soft-deleted or replaced — pick during build).
- [ ] ⬜ **Export breakdown:** PDF (via the same renderer chosen in Phase 0.3 — same toolchain, different template) + JSON.
- [ ] ⬜ **Admin debug view:** `/admin/breakdowns` — recent `aristotle_job` rows with timing, status, error if any, link to the resulting breakdown. Replaces aristotle's old standalone web UI.
- [ ] ⬜ **Tests at `server/tests/breakdown_pipeline_test.rs`:**
  - Happy path: invoke handler with a fixture PDF, assert `aristotle_job` flips through `queued → running → complete` and `scene`/`breakdown_item` rows land
  - Failure path: corrupt bytes → job ends `failed` with useful error string in `aristotle_job.error`
  - Concurrency: spawn N > semaphore-permits jobs, assert they serialize without errors
  - State-machine guard: `complete` job can't go back to `running` (illegal transition rejected)
  - Re-run: triggering breakdown on a script that already has one creates a new `aristotle_job` row and supersedes old scenes
  - Permission denied: non-admin / non-member can't trigger breakdown
  - Flag-off: `script_breakdown` flag `off` → trigger button absent, POST returns 404
- [ ] ⬜ **Tests at `server/tests/breakdown_review_routes_test.rs`** — render breakdown editor (renders correctly for paid+complete job), edit-chip POST updates DB, "approve scene" flips `scene.status`, permission denied path.

### 1.5 Script sharing
- [ ] ⬜ **Sharing model:** reuse `production_script.visibility` field. Values: `public`, `members`, `cast`, `crew`, `specific` (new). For `specific`, add `script_share { script, person }` edge.
- [ ] ⬜ **Migration: script_share** — `db/migrations/016_script_share.surql`
- [ ] ⬜ **Share UI:** "Share" button on each script version → multiselect from production members (auto-populated from `member_of`)
- [ ] ⬜ **Notification on upload:** new `script_uploaded` notification → all members with read access to that script's visibility scope
- [ ] ⬜ **Email digest (optional):** "New script available for {production}" with deep link
- [ ] ⬜ **Read tracking:** `script_view { script, person, viewed_at }` so owner can see who's seen the latest revision
- [ ] ⬜ **Add to cascade:** delete script_share and script_view rows in `Person::delete_with_cascade`
- [ ] ⬜ **Tests at `server/tests/script_share_test.rs`** — upload script → notifications created for in-scope members; visibility = `specific` only notifies listed people; revoking share removes future read access (but past notification stays for audit); read tracking sets `viewed_at` exactly once per (script, person); cascade test extended for `script_share` + `script_view` person refs.

### 1.6 Phase 1 sign-off
Flags have been at `admin_only` since Phase 0.2, so admins have been verifying work in-place throughout Phase 1. This step is just the final sign-off, not a flag flip.

- [ ] ⬜ All 1.1–1.5 boxes ticked
- [ ] ⬜ Slatehub-admin user (you) runs through the full happy path on a real production end-to-end
- [ ] ⬜ No P1 bugs open
- [ ] ⬜ Announce internally — invite other slatehub-admin team members to dogfood
- [ ] ⬜ Stay at `admin_only` until §4.6 promotion criteria are met for promotion to `verified`

**Phase 1 exit criteria:** Slatehub-admin owner of a production can see Overview/Management toggle. From management they can upload a script, see version history, click "Run breakdown", get notification when it's done, review and adjust the breakdown, share with cast/crew, and cast/crew get notified. Series productions show an episode list and management nests under episodes. **Non-admin users see no UI change.**

---

## Phase 2 — Scheduling & Call Sheets (weeks 6–9)

**Goal:** producers can plan shoot days, bin scenes onto days, generate and email call sheets.

### 2.1 Schedule data model
- [ ] ⬜ Implement `models/schedule.rs` — CRUD for `schedule_day`, `schedule_scene`, `call_time`
- [ ] ⬜ Crew availability check: prevent double-booking (one person can't have two `call_time` rows for overlapping shoot windows)
- [ ] ⬜ Episode-scoped scheduling for series

### 2.2 Schedule UI
- [ ] ⬜ **Calendar view:** month grid showing shoot days, color-coded by status
- [ ] ⬜ **Day view:** scenes for the day with start times, locations, scene durations
- [ ] ⬜ **Drag-and-drop scene assignment:** from "unscheduled" pool onto days (start simple: dropdown to assign scene → day → order)
- [ ] ⬜ **Auto-schedule suggestion** (deferred): use aristotle's bin-packing algo to suggest a baseline schedule, user adjusts
- [ ] ⬜ **iCal export:** subscribable feed `/productions/{slug}/schedule.ics`

### 2.3 Call sheets
*(PDF rendering technology already chosen in Phase 0.3 — see `docs/CALL_SHEET_PDF.md`.)*
- [ ] ⬜ **CallSheetContext struct:** the canonical data model passed to both the HTML preview and the PDF renderer. Fields per §2.7 (header, scenes[], cast[], crew[], locations[], emergency, distribution, notes).
- [ ] ⬜ **Generator:** assemble `CallSheetContext` from a `schedule_day` + linked `scene` + `breakdown_item` + `call_time` rows. Pull weather/sun-times if API configured; otherwise leave fields blank.
- [ ] ⬜ **Production-branded header:** logo from `production.poster_photo` (or fall back to slatehub logo), production name, episode (if series).
- [ ] ⬜ **PDF rendering:** flesh out `services/call_sheet_pdf.rs` (scaffolded in Phase 0.3) — store result as S3 key in `call_sheet.pdf_key`.
- [ ] ⬜ **HTML preview:** same context drives `templates/productions/manage/call_sheet_preview.html`. Side-by-side preview/PDF download in the management UI.
- [ ] ⬜ **Versioning:** each generate creates a new `call_sheet` row with incremented `version`; previous PDFs remain in S3 for audit.
- [ ] ⬜ **Send action:** "Publish & Email" button → background task sends email via `EmailService` to all `call_time.person` recipients with PDF attached + in-app `call_sheet_sent` notification.
- [ ] ⬜ **Regenerate guardrail:** if you regenerate after publish, warn explicitly — the previous version was already distributed, so the new one needs a fresh "Send" to reach people.
- [ ] ⬜ **Visual QA:** render call sheets for 3 representative shoot days (small indie, mid-budget feature, episodic TV) and review the output before flipping the `call_sheet_email` flag past `admin_only`.

### 2.4 Tests

All under `server/tests/`. **No inline tests anywhere.**

- [ ] ⬜ `server/tests/schedule_test.rs` — create schedule_day, assign scenes, assign call times; cascade on person delete
- [ ] ⬜ `server/tests/schedule_routes_test.rs` — happy path + permission denied + flag-off
- [ ] ⬜ `server/tests/schedule_conflict_test.rs` — double-book a crew member → validation error; same-person two overlapping shoots → rejected; identical call time same shoot day → coalesced or rejected (decide during build)
- [ ] ⬜ `server/tests/call_sheet_test.rs` — generate sheet from a fully-populated schedule, assert all required sections populated; missing weather → graceful render
- [ ] ⬜ `server/tests/call_sheets_routes_test.rs` — generate, preview, publish, send. Mocked email service.
- [ ] ⬜ `server/tests/call_sheet_send_test.rs` — "Publish & Email" enqueues correct recipients (only `call_time.person` for that day, deduplicated); email contains PDF attachment; in-app `call_sheet_sent` notifications created
- [ ] ⬜ Extend `server/tests/cascade_delete_test.rs` — new `call_time` rows for deleted person are removed; `call_sheet.generated_by` / `sent_by` set to NULL
- [ ] ⬜ End-to-end test `server/tests/production_lifecycle_test.rs` (first version) — production → upload → breakdown → schedule → call sheet → notifications, all in one test, mocked aristotle + mocked email

### 2.5 Phase 2 sign-off
Flags already at `admin_only` from Phase 0.2 — no flag flip required, just sign-off.

- [ ] ⬜ All 2.1–2.4 boxes ticked
- [ ] ⬜ Visual QA pass on call sheet PDF for 3 representative shoot days
- [ ] ⬜ One real shoot day's call sheet sent + received by a slatehub admin

**Phase 2 exit criteria:** schedule a shoot day, place scenes on it, assign call times, generate a call sheet, hit "Send" — every recipient gets an email with PDF and an in-app notification. Still gated to slatehub admins (`production_management` + `call_sheet_email` both at `admin_only`).

---

## Phase 3 — Casting + Locations + Team (weeks 10–13)

**Goal:** the spec's "Pre-production" features — characters, casting, locations, team onboarding.

### 3.1 Character bible
- [ ] ⬜ `character` table: `production, episode?, name, description, age_range, gender, ethnicity[], physical_attributes, arc_notes`
- [ ] ⬜ Per-script breakdown auto-populates character entries from `breakdown_item.category = "cast"`
- [ ] ⬜ Character detail page with photo refs and casting status
- [ ] ⬜ Cast attachment: link `person` (from slatehub directory) to `character` (offer / attached / confirmed)

### 3.2 Casting integration
- [ ] ⬜ Auto-suggest panel: given a character's age/gender/ethnicity, query slatehub `person` table, surface top candidates by `acting_age_range`, `acting_ethnicities`, location proximity
- [ ] ⬜ Casting call creation: posts to existing `job_posting` table tagged `cast` — shows up in slatehub Jobs
- [ ] ⬜ Self-tape: video upload via existing media handling, attached to `casting_audition` row
- [ ] ⬜ Audition scheduler: integrates with the production calendar

### 3.3 Locations integration
- [ ] ⬜ Link `scene.location_record` → existing `location` table when location is identified
- [ ] ⬜ Location-specific schedule view: which scenes are at this location across the schedule
- [ ] ⬜ Location booking calendar (basic): mark a location "held" for date ranges

### 3.4 Team onboarding
- [ ] ⬜ Bulk invite: paste emails or pick from SlateHub directory
- [ ] ⬜ Deal-memo template (PDF generated, signed offline for v1, e-sign later)
- [ ] ⬜ Onboarding checklist per role (tax forms, contact info, NDA acknowledgment)

**Phase 3 exit criteria:** producers can build a character list, send casting calls, hold locations, and onboard crew without leaving slatehub.

---

## Phase 4 — Production Live + WhatsApp (weeks 14–17)

**Goal:** on-set usefulness + WhatsApp delivery channel.

### 4.1 On-set tools
- [ ] ⬜ Scene status board (`not started` / `shooting` / `wrapped`) — quick toggle on call sheet page
- [ ] ⬜ Daily progress log: free-text field per `schedule_day` + per `scene` (notes from script supervisor)
- [ ] ⬜ Photo/video upload tagged to scene (BTS, continuity)
- [ ] ⬜ Digital sides: extracted scene pages → PDF for the day
- [ ] ⬜ Safety checklist per shoot day

### 4.2 Mobile-friendly views
- [ ] ⬜ Responsive call sheet view (already needed for on-set phone use)
- [ ] ⬜ "Today's call sheet" route: `/productions/{slug}/today` — single page everyone bookmarks
- [ ] ⬜ Push notifications (web push) for schedule changes

### 4.3 WhatsApp delivery
- [ ] ⬜ Survey state of `whatsapp-bot/` directory — what's working, what's stubbed
- [ ] ⬜ WhatsApp Business API: set up sender number, register message templates (template approval is a 1–2 week external process)
- [ ] ⬜ Per-user WhatsApp opt-in setting (`person.notification_preferences.whatsapp` + phone number verification)
- [ ] ⬜ Call sheet delivery via WhatsApp template + PDF document
- [ ] ⬜ Schedule-change broadcast to crew with WhatsApp opted-in

**Phase 4 exit criteria:** on shoot day, the AD updates scene status from their phone, the crew gets call sheets via WhatsApp, and end-of-day photos flow back into the production record.

---

## Phase 5 — Post / Marketing / Distribution (weeks 18+)

**Goal:** complete the lifecycle.

### 5.1 Asset library
- [ ] ⬜ Generic `asset` table: `production, episode?, scene?, type, name, s3_key, uploaded_by, tags[]`
- [ ] ⬜ Asset categories: footage, audio, vfx_shot, graphic, behind_scenes, edit_version, marketing
- [ ] ⬜ Folder/hierarchy view + tag filter

### 5.2 Review & approval workflows
- [ ] ⬜ `review_round` table: `asset, requested_by, requested_at, status, notes`
- [ ] ⬜ Approval chain: Director's cut → Producer → Studio (configurable per production)
- [ ] ⬜ Time-coded comments on video (deferred unless we add a real video player)

### 5.3 VFX / Music / Sound tracking
- [ ] ⬜ VFX shot list — derived from `breakdown_item.category = "vfx"`, with vendor + delivery status
- [ ] ⬜ Music spotting sheet
- [ ] ⬜ Sound effects list

### 5.4 Festival / distribution
- [ ] ⬜ `festival_submission` table with deadlines + status
- [ ] ⬜ `distribution_deal` simple CRM (vendor, platform, status, terms_notes)
- [ ] ⬜ Marketing assets hub (already a kind of asset, just a tagged view)

### 5.5 Budget (Phase 5 because it's the most boring and least urgent)
- [ ] ⬜ `budget_line` table: department, description, estimated, actual
- [ ] ⬜ Department-grouped budget view
- [ ] ⬜ CSV export

---

## 4. Cross-cutting concerns

### 4.1 Notifications
Tracked per-phase but worth listing globally — new `notification_type` values to add:
| Type | Phase | Trigger |
|---|---|---|
| `script_uploaded` | 1 | new version uploaded |
| `script_shared` | 1 | granted access to a script |
| `breakdown_ready` | 1 | aristotle finished |
| `schedule_changed` | 2 | shoot day edited within 48h of shoot |
| `call_sheet_sent` | 2 | daily sheet published |
| `cast_offer` | 3 | character attached |
| `audition_scheduled` | 3 | audition slot booked |
| `asset_review_requested` | 5 | review round opened |

### 4.2 Activity log
Every meaningful action writes to `activity_event` for audit. Already-existing infra.

### 4.3 Testing strategy

**All tests under `server/tests/`. Never inline `#[cfg(test)] mod tests`.** See §2.X "Tests" for the hard rule and rationale.

**File layout — one test file per source module:**

| Source | Test file |
|---|---|
| `models/episode.rs` | `tests/episode_test.rs` |
| `models/season.rs` | `tests/season_test.rs` |
| `models/scene.rs` | `tests/scene_test.rs` |
| `models/breakdown.rs` | `tests/breakdown_test.rs` |
| `models/schedule.rs` | `tests/schedule_test.rs` |
| `models/call_sheet.rs` | `tests/call_sheet_test.rs` |
| `services/aristotle_runner.rs` | `tests/aristotle_runner_test.rs` |
| `services/call_sheet_pdf.rs` | `tests/call_sheet_pdf_test.rs` |
| `routes/episodes.rs` | `tests/episode_routes_test.rs` |
| `routes/breakdown_review.rs` | `tests/breakdown_review_routes_test.rs` |
| `routes/schedule.rs` | `tests/schedule_routes_test.rs` |
| `routes/call_sheets.rs` | `tests/call_sheets_routes_test.rs` |
| `aristotle/` crate (parser + breakdown library) | `aristotle/tests/*.rs` — already exists, keeps its own tests |

**Test categories required for every feature:**

1. **Happy path** — the canonical use case end-to-end
2. **Authorization** — non-admin / non-member / wrong-role each rejected
3. **Validation** — missing fields, malformed input, schema-violating values rejected with useful errors
4. **State transitions** — for any status field, exercise legal transitions and assert illegal ones are rejected
5. **Cascade behaviour** — extend `tests/cascade_delete_test.rs` if the feature adds person-linked rows
6. **Idempotency** — webhooks / retries / refresh-clicks don't double-write
7. **Concurrency** — for anything backed by a semaphore or `tokio::spawn`, assert it serializes correctly

**Existing tests stay green.** Don't tick a Phase task as complete until `cargo test --tests` passes the whole suite, not just the new file.

**Aristotle library tests** live with the crate at `aristotle/tests/` (the existing 23 tests, minus dropped HTTP tests). Slatehub-side tests exercise the wrapper in `services/aristotle_runner.rs`, not aristotle's internals directly — those are aristotle's responsibility.

**End-to-end golden path** lives at `tests/production_lifecycle_test.rs` once Phase 2 ships: create production → invite member → upload script → run breakdown → review → schedule a shoot day → publish call sheet → verify notifications + email + DB state. Mocked aristotle (via test fixture) + mocked email in CI. **One test, end of every phase**, kept green forever.

### 4.4 GDPR cascade
**Every new table that references `person`** must be added to `Person::delete_with_cascade` in `models/person.rs`. Tests in `tests/cascade_delete_test.rs` must be extended. Tables added in this plan that need cascade entries:
- `schedule_scene` (no person ref) — skip
- `call_time` (person ref) — DELETE
- `call_sheet` (generated_by, sent_by) — NULL out
- `script_share`, `script_view` — DELETE
- `aristotle_job` (no person ref directly — script.uploaded_by is the link) — already covered via production_script
- `character` — production-owned, not person-owned, no cascade needed
- `casting_audition` — DELETE
- `festival_submission`, `distribution_deal` — production-owned, no cascade

### 4.5 Performance / scale
- Productions list page already does embedding search — fine.
- Schedule queries are by date range per production — needs an index on `schedule_day(production, date)`.
- Breakdown queries can be heavy for long scripts — paginate the scene list, lazy-load chips.
- Aristotle job polling: stagger to avoid stampede.

### 4.6 Feature flag rollout

**Initial state — `admin_only` from the moment each flag is registered.** This lets the developer (a slatehub admin) verify each task in-place without manual flag toggles after every deploy. Non-admins still see nothing — the gate just lets admins through from day one.

| Flag | Registered state (Phase 0) | Beta (Phase 2/3) | GA |
|---|---|---|---|
| `production_management` | **`admin_only`** | `verified` (opt-in cohort) | `all` |
| `script_breakdown` | **`admin_only`** | `verified` | `all` |
| `call_sheet_email` | **`admin_only`** | `verified` | `all` |

The `production_management` flag is the **master switch**. Sub-feature flags (`script_breakdown`, `call_sheet_email`) layer on top — you can leave the master at `admin_only` while flipping a sub-flag to `verified` once that subsystem is proven. Order matters: keep the master no more permissive than any sub-flag, otherwise non-admins can hit a feature gate they shouldn't see.

**Who counts as a slatehub admin during initial rollout** — anyone with `person.is_admin = true`. Set via `/admin/people` (existing toggle). For first ship we'd have a handful of internal users + maybe one trusted producer.

**Promotion criteria — explicit gates before flipping past `admin_only`:**
1. All Phase 1 exit criteria met
2. At least one full production run end-to-end by a slatehub-admin user (real script → real breakdown → real schedule → real call sheet sent + received)
3. No P1 bugs open
4. Visual-QA pass on the management UI at three screen sizes (mobile, tablet, desktop)
5. Stripe-style refund-on-failure tested for any paid features (none yet, but the pattern is set)
6. `/admin/feature-flags` admin can flip the state back to `admin_only` instantly if anything goes wrong (already supported by the system)

**No-flag access fallback** — productions remain *viewable* in their existing public form regardless of flag state. The flag gates the **management workspace**, not production data itself. Critically: if we flip `production_management` to `off` after going live, paying customers don't lose data; they just temporarily can't manage. Make this explicit in any future communication.

### 4.7 Aristotle sidecar deployment
- [ ] ⬜ Document in `docs/ARISTOTLE_SIDECAR.md`:
  - How to build and run aristotle alongside slatehub (docker-compose)
  - Required env vars and secrets
  - LLM backend choice (Ollama for self-hosted, Anthropic for managed)
  - Webhook callback URL configuration
  - Health-check + monitoring expectations
- [ ] ⬜ Add an aristotle health endpoint check at server boot — log warning if unreachable (don't refuse to start)

---

## 5. Risks & open questions

| # | Risk / Question | Mitigation / Decision |
|---|---|---|
| R1 | ~~Aristotle's API is currently un-authenticated~~ — N/A. We're not using aristotle as a sidecar; it's an in-process crate. No HTTP surface to defend. |
| R2 | LLM cost if we enable Tier 4 | Default `ARISTOTLE_POLICY=deterministic` (no LLM); gate Tier 4 behind a per-production feature flag |
| R2b | Heavy parsing deps bloat the slatehub binary + container image | Acceptable cost for the operational simplicity gain. Audit `cargo bloat` after the workspace conversion; consider feature flags on aristotle crate to drop unused parsers (e.g. omit Fade In if we never see one) |
| R2c | CPU-bound breakdown can starve HTTP threads | Mitigated by `tokio::sync::Semaphore` cap in `aristotle_runner` + by wrapping pure-sync parser calls in `tokio::task::spawn_blocking` so they don't sit on the cooperative-scheduling runtime |
| R3 | WhatsApp template approval can take 1–2 weeks | Start template submission at the beginning of Phase 4, not during |
| R4 | Series with many episodes could make UI heavy | Episode list paginates; default to "most recent + in-progress" view |
| R5 | Schedule conflicts across multiple productions for one crew member | Out of scope for v1 — single-production scope only; cross-production view is a Phase 5+ feature |
| R6 | PDF call-sheet rendering quality (HIGH PRIORITY — producers judge tools by this) | **Moved to Phase 0.3** — done before any other call-sheet work. Spike Typst vs. headless Chromium against a StudioBinder-style reference; pick the winner; document in `docs/CALL_SHEET_PDF.md`. Tentative pick: Typst. |
| R7 | Production owners want StudioBinder-like polish | Phase 1 ships functional, not gorgeous. Phase 3 includes a design pass on the management shell after we've learned what people actually use. |
| R8 | Spec mentions "real-time collaborative" — does that mean WebSocket? | v1: server-rendered + Datastar SSE for live updates (consistent with rest of slatehub). True multi-user editing of breakdowns deferred to Phase 5+. |
| Q1 | Should episodes have their own URL slug or live under production? | Decision: `/productions/{prod_slug}/episodes/{ep_slug}` — keeps episode discovery scoped to its parent |
| Q2 | Where do scripts attach for series — production root or episode? | Both — `production_script.episode` is nullable; if set, script is episode-scoped; if null, it's a series-level reference (pilot bible, format bible) |
| Q3 | Do we need a CDN for script PDFs? | Defer — S3 with signed URLs is fine for v1; add CDN if download latency becomes a complaint |
| Q4 | What language(s) for call sheets? | English-only v1; spec says international users — add i18n in Phase 4 alongside WhatsApp rollout (templates need translated copy anyway) |

---

## 6. Timeline summary

| Phase | Weeks | Goal | Ships when |
|---|---|---|---|
| 0 | 1–2 | Schema + workspace conversion + aristotle crate + PDF spike | Migrations applied, `cargo test` green across workspace, `aristotle_runner` produces a fixture breakdown, no UI yet |
| 1 | 2–5 | Management mode, scripts, breakdown, sharing | A team can run a single production end-to-end through script + breakdown |
| 2 | 6–9 | Scheduling + email call sheets | A team can shoot a movie / episode |
| 3 | 10–13 | Casting + locations + team onboarding | Full pre-pro replaces external tools |
| 4 | 14–17 | On-set tools + WhatsApp | Production day workflow is mobile-first |
| 5 | 18+ | Post, marketing, distribution, budget | Full lifecycle in one place |

Working assumption: one developer-week per checkbox cluster (3–6 boxes). Adjust based on actual velocity; this is a budgeting heuristic.

---

## 7. How to use this doc

1. **As a plan:** read top-down to understand the shape.
2. **As a tracker:** flip the `⬜` to `🟡` when you start, `✅` when done. Add an inline date if useful.
3. **As a backlog:** Phase 5+ items are placeholders — refine when we get closer.
4. **When scope changes:** update the relevant phase section directly in this file (not a separate doc).
5. **When a decision is made on an open question:** strike the row out and add the resolution inline.

Open issues and PRs should reference the checkbox text (e.g., "closes the `Migration: episode table` box under 0.1") so the graph stays navigable.
