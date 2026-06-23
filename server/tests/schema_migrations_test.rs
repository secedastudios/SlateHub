//! Schema-level tests for the productions-management migrations (012–016).
//!
//! These exercise constraints (ASSERT clauses + UNIQUE indexes) on the
//! tables those migrations introduce, against the test DB which is
//! initialized from `db/schema.surql`. If a migration adds a constraint,
//! that constraint must be enforced here — otherwise the migration is
//! effectively cosmetic.
//!
//! Particularly important: the **nullable-season episode uniqueness**
//! semantics from project plan §2.2 — flat episodes (`season = NULL`)
//! and seasoned episodes must coexist correctly under one UNIQUE index.

mod common;

use slatehub::db::DB;
use surrealdb::types::{RecordId, SurrealValue};

#[derive(serde::Deserialize, SurrealValue)]
struct R {
    id: RecordId,
}

async fn seed_production(slug: &str) -> RecordId {
    let rows: Vec<R> = DB
        .query(
            "CREATE production CONTENT {
                title: $slug,
                slug: $slug,
                type: 'Feature Film',
                status: 'in_development'
            } RETURN id",
        )
        .bind(("slug", slug.to_string()))
        .await
        .expect("create production")
        .take(0)
        .expect("take production");
    rows.into_iter().next().expect("one production").id
}

async fn seed_season(production: &RecordId, season_number: i64) -> RecordId {
    let rows: Vec<R> = DB
        .query("CREATE season SET production = $p, season_number = $n RETURN id")
        .bind(("p", production.clone()))
        .bind(("n", season_number))
        .await
        .expect("create season")
        .take(0)
        .expect("take season");
    rows.into_iter().next().expect("one season").id
}

fn clean() {
    for table in [
        "aristotle_job",
        "call_sheet",
        "call_time",
        "schedule_scene",
        "schedule_day",
        "breakdown_item",
        "scene",
        "episode",
        "season",
        "production",
    ] {
        common::clean_table(table);
    }
}

// ---------------------------------------------------------------------------
// season constraints
// ---------------------------------------------------------------------------

#[test]
fn season_unique_per_production_number() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("season-unique").await;

        DB.query("CREATE season SET production = $p, season_number = 1")
            .bind(("p", prod.clone()))
            .await
            .expect("first season ok");

        // Duplicate (production, season_number) — UNIQUE index must reject.
        let r = DB
            .query("CREATE season SET production = $p, season_number = 1")
            .bind(("p", prod.clone()))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "duplicate (production, season_number) must be rejected"
        );
    });
}

#[test]
fn season_status_assert_rejects_invalid_value() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("season-status").await;
        let r = DB
            .query("CREATE season SET production = $p, season_number = 1, status = 'banana'")
            .bind(("p", prod))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "season.status outside the enum must be rejected by the ASSERT"
        );
    });
}

// ---------------------------------------------------------------------------
// episode uniqueness — every episode belongs to a season (project plan §2.2)
// ---------------------------------------------------------------------------

#[test]
fn episode_requires_a_season() {
    // SurrealDB v3 NULL-uniqueness semantics would allow duplicate flat
    // episodes if season were nullable. Schema makes it required so the
    // (season, episode_number) UNIQUE index is meaningful. Flat series
    // get an auto-created "Season 1" at the application layer.
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("ep-needs-season").await;
        let r = DB
            .query(
                "CREATE episode SET production = $p, episode_number = 1, \
                 title = 'No season', slug = 'no-season'",
            )
            .bind(("p", prod))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "creating an episode without a season must fail (TYPE record<season> is required)"
        );
    });
}

#[test]
fn episode_uniqueness_within_a_season() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("ep-within-season").await;
        let s1 = seed_season(&prod, 1).await;

        DB.query(
            "CREATE episode SET production = $p, season = $s, episode_number = 1, \
             title = 'S1E1', slug = 's1e1'",
        )
        .bind(("p", prod.clone()))
        .bind(("s", s1.clone()))
        .await
        .expect("first S1E1 ok");

        let r = DB
            .query(
                "CREATE episode SET production = $p, season = $s, episode_number = 1, \
                 title = 'S1E1 dup', slug = 's1e1-dup'",
            )
            .bind(("p", prod))
            .bind(("s", s1))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "two episodes with same number in the same season must be rejected"
        );
    });
}

#[test]
fn episode_number_one_can_repeat_across_different_seasons() {
    // S1E1 and S2E1 are distinct — UNIQUE index is (season, episode_number),
    // not just (episode_number).
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("ep-across-seasons").await;
        let s1 = seed_season(&prod, 1).await;
        let s2 = seed_season(&prod, 2).await;

        DB.query(
            "CREATE episode SET production = $p, season = $s, episode_number = 1, \
             title = 'S1E1', slug = 's1e1'",
        )
        .bind(("p", prod.clone()))
        .bind(("s", s1))
        .await
        .expect("S1E1 ok");

        let result = DB
            .query(
                "CREATE episode SET production = $p, season = $s, episode_number = 1, \
                 title = 'S2E1', slug = 's2e1'",
            )
            .bind(("p", prod))
            .bind(("s", s2))
            .await;
        let ok = result.map(|r| r.check().is_ok()).unwrap_or(false);
        assert!(ok, "S1E1 and S2E1 must coexist — different seasons");
    });
}

#[test]
fn episode_slug_unique_per_production() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("ep-slug").await;
        let s1 = seed_season(&prod, 1).await;

        DB.query(
            "CREATE episode SET production = $p, season = $s, episode_number = 1, \
             title = 'A', slug = 'pilot'",
        )
        .bind(("p", prod.clone()))
        .bind(("s", s1.clone()))
        .await
        .expect("first slug ok");

        let r = DB
            .query(
                "CREATE episode SET production = $p, season = $s, episode_number = 2, \
                 title = 'B', slug = 'pilot'",
            )
            .bind(("p", prod))
            .bind(("s", s1))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "duplicate (production, slug) must be rejected"
        );
    });
}

#[test]
fn episode_slug_must_be_non_empty() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("ep-slug-empty").await;
        let s1 = seed_season(&prod, 1).await;
        let r = DB
            .query(
                "CREATE episode SET production = $p, season = $s, episode_number = 1, \
                 title = 'A', slug = ''",
            )
            .bind(("p", prod))
            .bind(("s", s1))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "empty episode.slug must be rejected by the ASSERT"
        );
    });
}

// ---------------------------------------------------------------------------
// scene + breakdown_item constraints
// ---------------------------------------------------------------------------

#[test]
fn breakdown_item_category_assert_rejects_unknown_category() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("bd-cat").await;
        let scene_rows: Vec<R> = DB
            .query(
                "CREATE scene SET production = $p, scene_number = '1', heading = 'INT. ROOM' RETURN id",
            )
            .bind(("p", prod))
            .await
            .expect("create scene")
            .take(0)
            .expect("take scene");
        let scene = scene_rows.into_iter().next().expect("one scene").id;

        let r = DB
            .query("CREATE breakdown_item SET scene = $s, category = 'asteroid', value = 'X'")
            .bind(("s", scene))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "unknown breakdown_item.category must be rejected"
        );
    });
}

// ---------------------------------------------------------------------------
// schedule + call_time + call_sheet constraints
// ---------------------------------------------------------------------------

#[test]
fn schedule_scene_unique_day_scene() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("sched").await;
        let day_rows: Vec<R> = DB
            .query("CREATE schedule_day SET production = $p, date = time::now() RETURN id")
            .bind(("p", prod.clone()))
            .await
            .expect("create day")
            .take(0)
            .expect("take day");
        let day = day_rows.into_iter().next().expect("one day").id;

        let scene_rows: Vec<R> = DB
            .query(
                "CREATE scene SET production = $p, scene_number = '1', heading = 'INT. ROOM' RETURN id",
            )
            .bind(("p", prod))
            .await
            .expect("create scene")
            .take(0)
            .expect("take scene");
        let scene = scene_rows.into_iter().next().expect("one scene").id;

        DB.query("CREATE schedule_scene SET schedule_day = $d, scene = $s, order_index = 0")
            .bind(("d", day.clone()))
            .bind(("s", scene.clone()))
            .await
            .expect("first placement ok");

        let r = DB
            .query("CREATE schedule_scene SET schedule_day = $d, scene = $s, order_index = 1")
            .bind(("d", day))
            .bind(("s", scene))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "same scene placed twice on the same day must be rejected"
        );
    });
}

#[test]
fn call_time_unique_day_person() {
    common::setup_test_db();
    clean();
    common::clean_table("person");
    common::clean_table("call_time");

    common::run(async {
        // Create a person directly (test isolation — no slatehub auth path).
        let person_rows: Vec<R> = DB
            .query(
                "CREATE person CONTENT {
                    username: 'ct_user', email: 'ct@x.test', password: 'h', name: 'x',
                    verification_status: 'unverified',
                    profile: { name: 'x', skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
                } RETURN id",
            )
            .await
            .expect("create person")
            .take(0)
            .expect("take person");
        let person = person_rows.into_iter().next().expect("one person").id;

        let prod = seed_production("call-time").await;
        let day_rows: Vec<R> = DB
            .query("CREATE schedule_day SET production = $p, date = time::now() RETURN id")
            .bind(("p", prod))
            .await
            .expect("create day")
            .take(0)
            .expect("take day");
        let day = day_rows.into_iter().next().expect("one day").id;

        DB.query("CREATE call_time SET schedule_day = $d, person = $p")
            .bind(("d", day.clone()))
            .bind(("p", person.clone()))
            .await
            .expect("first call_time ok");

        let r = DB
            .query("CREATE call_time SET schedule_day = $d, person = $p")
            .bind(("d", day))
            .bind(("p", person))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "two call_time rows for same (day, person) must be rejected"
        );

        // Cleanup via raw DELETE — common::clean_table uses block_on and
        // can't be called from inside an existing runtime.
        let _ = DB.query("DELETE person").await;
    });
}

#[test]
fn call_sheet_version_unique_per_day() {
    common::setup_test_db();
    clean();

    common::run(async {
        let prod = seed_production("call-sheet").await;
        let day_rows: Vec<R> = DB
            .query("CREATE schedule_day SET production = $p, date = time::now() RETURN id")
            .bind(("p", prod))
            .await
            .expect("create day")
            .take(0)
            .expect("take day");
        let day = day_rows.into_iter().next().expect("one day").id;

        DB.query("CREATE call_sheet SET schedule_day = $d, version = 1, pdf_key = 'k1'")
            .bind(("d", day.clone()))
            .await
            .expect("v1 ok");

        let r = DB
            .query("CREATE call_sheet SET schedule_day = $d, version = 1, pdf_key = 'k1-dup'")
            .bind(("d", day))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "two call_sheet rows with the same (day, version) must be rejected"
        );
    });
}

// ---------------------------------------------------------------------------
// aristotle_job state ASSERT
// ---------------------------------------------------------------------------

#[test]
fn aristotle_job_status_assert_rejects_invalid() {
    common::setup_test_db();
    clean();
    common::clean_table("production_script");
    common::clean_table("person");

    common::run(async {
        let prod = seed_production("aj-state").await;

        // Create the uploader first, separately — avoids the parser
        // confusion that comes from nesting CREATE inside RETURN VALUE.
        let person_rows: Vec<R> = DB
            .query(
                "CREATE person CONTENT {
                    username: 'aj_user', email: 'aj@x.test', password: 'h', name: 'x',
                    verification_status: 'unverified',
                    profile: { name: 'x', skills: [], social_links: [], ethnicity: [], unions: [], languages: [], experience: [], education: [], reels: [], media_other: [], awards: [] }
                } RETURN id",
            )
            .await
            .expect("create person")
            .take(0)
            .expect("take person");
        let person = person_rows.into_iter().next().expect("one person").id;

        let script_rows: Vec<R> = DB
            .query(
                "CREATE production_script SET production = $p, title = 'S', version = 1, \
                 file_url = 'u', file_key = 'k', file_size = 1, mime_type = 'application/pdf', \
                 visibility = 'members', uploaded_by = $u RETURN id",
            )
            .bind(("p", prod.clone()))
            .bind(("u", person))
            .await
            .expect("create script")
            .take(0)
            .expect("take script");
        let script = script_rows.into_iter().next().expect("one script").id;

        let r = DB
            .query("CREATE aristotle_job SET production = $p, script = $s, status = 'martian'")
            .bind(("p", prod))
            .bind(("s", script))
            .await;
        assert!(
            r.is_err() || r.unwrap().check().is_err(),
            "aristotle_job.status outside the enum must be rejected"
        );

        // Raw DELETE — common::clean_table uses block_on and panics from
        // inside common::run.
        let _ = DB.query("DELETE production_script").await;
        let _ = DB.query("DELETE person").await;
    });
}
