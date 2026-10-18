//! Tests for the boot-time schema migration runner (`db::migrations`).
//!
//! Pure checks (embedded list ↔ directory agreement, naming, the
//! destructive-statement guard, checksums) need no DB. The runner tests use
//! the shared test database and clean up their own ledger rows and probe
//! tables, using names (`9xx_…`) that no real migration will ever take.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use slatehub::db::DB;
use slatehub::db::migrations::{self, BASELINE, MIGRATIONS, Migration, MigrationError};

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

// ---------------------------------------------------------------------------
// Pure: the shipped list
// ---------------------------------------------------------------------------

#[test]
fn embedded_list_matches_the_migrations_directory() {
    let on_disk: BTreeSet<String> = std::fs::read_dir(migrations_dir())
        .expect("server/migrations exists")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "surql"))
        .map(|p| p.file_stem().expect("stem").to_string_lossy().into_owned())
        .collect();
    let embedded: BTreeSet<String> = MIGRATIONS.iter().map(|m| m.name.to_string()).collect();
    assert_eq!(
        embedded, on_disk,
        "every server/migrations/*.surql must be registered in MIGRATIONS (and vice versa)"
    );

    for m in MIGRATIONS {
        let path = migrations_dir().join(format!("{}.surql", m.name));
        let contents = std::fs::read_to_string(&path).expect("read migration file");
        assert_eq!(
            m.sql, contents,
            "{} embeds a different file than the one on disk",
            m.name
        );
    }
}

#[test]
fn shipped_migrations_are_valid_and_non_destructive() {
    migrations::validate(MIGRATIONS).expect("MIGRATIONS list is well-formed");
    for m in MIGRATIONS {
        migrations::check_destructive(m).expect("shipped migrations must not remove data");
    }
}

// ---------------------------------------------------------------------------
// Pure: validation + the destructive guard
// ---------------------------------------------------------------------------

fn is_invalid(r: Result<(), MigrationError>) -> bool {
    matches!(r, Err(MigrationError::Invalid(_)))
}

#[test]
fn validate_rejects_bad_names_baseline_numbers_and_misordering() {
    let bad_name = [Migration {
        name: "fifteen_thing",
        sql: "RETURN 1;",
    }];
    assert!(is_invalid(migrations::validate(&bad_name)));

    let baseline = [Migration {
        name: "014_already_squashed",
        sql: "RETURN 1;",
    }];
    assert!(
        is_invalid(migrations::validate(&baseline)),
        "numbers <= BASELINE ({BASELINE}) are part of schema.surql"
    );

    let unsorted = [
        Migration {
            name: "016_b",
            sql: "RETURN 1;",
        },
        Migration {
            name: "015_a",
            sql: "RETURN 1;",
        },
    ];
    assert!(is_invalid(migrations::validate(&unsorted)));

    let duplicate = [
        Migration {
            name: "015_a",
            sql: "RETURN 1;",
        },
        Migration {
            name: "015_a",
            sql: "RETURN 2;",
        },
    ];
    assert!(is_invalid(migrations::validate(&duplicate)));

    let comments_only = [Migration {
        name: "015_a",
        sql: "  \n-- nothing but a comment\n",
    }];
    assert!(is_invalid(migrations::validate(&comments_only)));

    let own_txn = [Migration {
        name: "015_a",
        sql: "BEGIN TRANSACTION; RETURN 1; COMMIT TRANSACTION;",
    }];
    assert!(is_invalid(migrations::validate(&own_txn)));

    let fine = [
        Migration {
            name: "015_a",
            sql: "RETURN 1;",
        },
        Migration {
            name: "016_b",
            sql: "RETURN 2;",
        },
    ];
    migrations::validate(&fine).expect("ascending, well-named, non-empty list is valid");
}

#[test]
fn destructive_statements_require_an_authorization_line() {
    for sql in [
        "DELETE person WHERE x = 1;",
        "REMOVE TABLE person;",
        "REMOVE FIELD foo ON person;",
        "UPDATE person UNSET foo;",
        "remove database main;",
        "REMOVE NAMESPACE slatehub;",
    ] {
        let m = Migration {
            name: "900_nope",
            sql,
        };
        assert!(
            matches!(
                migrations::check_destructive(&m),
                Err(MigrationError::Unauthorized { .. })
            ),
            "{sql:?} must be refused without authorization"
        );
    }
}

#[test]
fn destructive_words_in_comments_and_index_removal_are_fine() {
    let prose = Migration {
        name: "900_ok",
        sql: "-- we never DELETE rows here, nor REMOVE TABLE anything\n\
              DEFINE FIELD OVERWRITE x ON person TYPE option<string>;",
    };
    migrations::check_destructive(&prose).expect("comments are ignored");

    let index = Migration {
        name: "900_ok",
        sql: "REMOVE INDEX IF EXISTS idx_x ON person;",
    };
    migrations::check_destructive(&index).expect("dropping an index loses no data");
}

#[test]
fn authorization_line_unlocks_destructive_statements() {
    let m = Migration {
        name: "900_authorized",
        sql: "-- destructive: authorized by Chris Bruce, 2026-10-18 — drop the legacy foo table\n\
              REMOVE TABLE foo;",
    };
    migrations::check_destructive(&m).expect("explicit authorization line is honoured");

    let vague = Migration {
        name: "900_vague",
        sql: "-- destructive: authorized by\nREMOVE TABLE foo;",
    };
    assert!(
        migrations::check_destructive(&vague).is_err(),
        "the line must name who authorized it"
    );
}

#[test]
fn checksum_ignores_line_ending_style() {
    assert_eq!(
        migrations::checksum("a;\nb;\n"),
        migrations::checksum("a;\r\nb;\r\n")
    );
    assert_ne!(migrations::checksum("a;"), migrations::checksum("b;"));
    assert_eq!(migrations::checksum("x").len(), 64, "hex-encoded sha256");
}

// ---------------------------------------------------------------------------
// Runner, against the test DB
// ---------------------------------------------------------------------------

async fn ledger_names() -> Vec<String> {
    let mut r = DB
        .query("SELECT VALUE name FROM schema_migration")
        .await
        .expect("query ledger");
    r.take::<Vec<String>>(0).expect("take names")
}

async fn reset(names: &[&str], probe_tables: &[&str]) {
    for n in names {
        DB.query("DELETE schema_migration WHERE name = $n")
            .bind(("n", n.to_string()))
            .await
            .expect("clean ledger");
    }
    for t in probe_tables {
        DB.query(format!("REMOVE TABLE IF EXISTS {t}"))
            .await
            .expect("drop probe table");
    }
}

#[test]
fn run_applies_pending_files_in_order_and_is_idempotent() {
    common::setup_test_db();
    common::run(async {
        reset(
            &["900_probe_table", "901_probe_field"],
            &["migration_probe"],
        )
        .await;
        let list = [
            Migration {
                name: "900_probe_table",
                sql: "DEFINE TABLE IF NOT EXISTS migration_probe SCHEMAFULL;\n\
                      DEFINE FIELD IF NOT EXISTS v ON migration_probe TYPE int;",
            },
            Migration {
                name: "901_probe_field",
                sql: "DEFINE FIELD OVERWRITE w ON migration_probe TYPE option<string>;\n\
                      CREATE migration_probe:seed SET v = 1;",
            },
        ];

        let first = migrations::run(&list).await.expect("first run applies");
        assert_eq!(first.applied, vec!["900_probe_table", "901_probe_field"]);
        assert!(first.already_applied.is_empty() && first.drifted.is_empty());

        // The files' effects are in place…
        let mut r = DB
            .query("SELECT VALUE v FROM migration_probe:seed")
            .await
            .expect("probe row");
        assert_eq!(r.take::<Option<i64>>(0).expect("take v"), Some(1));
        // …and both are in the ledger.
        let names = ledger_names().await;
        assert!(names.contains(&"900_probe_table".to_string()));
        assert!(names.contains(&"901_probe_field".to_string()));

        let second = migrations::run(&list).await.expect("second run is a no-op");
        assert!(second.applied.is_empty());
        assert_eq!(
            second.already_applied,
            vec!["900_probe_table", "901_probe_field"]
        );

        reset(
            &["900_probe_table", "901_probe_field"],
            &["migration_probe"],
        )
        .await;
    });
}

#[test]
fn run_rolls_back_a_failing_file_and_records_nothing() {
    common::setup_test_db();
    common::run(async {
        reset(&["902_bad"], &["migration_probe_bad"]).await;
        // Statements 1–3 are fine; statement 4 fails type coercion at runtime.
        let list = [Migration {
            name: "902_bad",
            sql: "DEFINE TABLE IF NOT EXISTS migration_probe_bad SCHEMAFULL;\n\
                  DEFINE FIELD IF NOT EXISTS v ON migration_probe_bad TYPE int;\n\
                  CREATE migration_probe_bad:ok SET v = 1;\n\
                  CREATE migration_probe_bad:bad SET v = 'not an int';",
        }];

        let err = migrations::run(&list)
            .await
            .expect_err("a failing statement must fail the run");
        assert!(
            matches!(err, MigrationError::Failed { ref name, .. } if name == "902_bad"),
            "got {err:?}"
        );

        assert!(
            !ledger_names().await.contains(&"902_bad".to_string()),
            "a failed migration must not be recorded as applied"
        );
        // DDL is transactional too: the whole file rolled back, so either the
        // table is gone (v3 then errors on the lookup) or at least the row is.
        let probe = DB
            .query("SELECT VALUE v FROM migration_probe_bad:ok")
            .await
            .expect("send probe")
            .check();
        let rolled_back = match probe {
            Err(e) => e.to_string().contains("does not exist"),
            Ok(mut r) => r.take::<Option<i64>>(0).expect("take v").is_none(),
        };
        assert!(
            rolled_back,
            "the table/row written before the failure must have been rolled back"
        );

        reset(&["902_bad"], &["migration_probe_bad"]).await;
    });
}

#[test]
fn run_refuses_an_unauthorized_destructive_file_before_touching_anything() {
    common::setup_test_db();
    common::run(async {
        reset(
            &["903_first", "904_destructive"],
            &["migration_probe_guard"],
        )
        .await;
        let list = [
            Migration {
                name: "903_first",
                sql: "DEFINE TABLE IF NOT EXISTS migration_probe_guard SCHEMAFULL;",
            },
            Migration {
                name: "904_destructive",
                sql: "DELETE person;",
            },
        ];
        let err = migrations::run(&list).await.expect_err("must refuse");
        assert!(
            matches!(err, MigrationError::Unauthorized { .. }),
            "got {err:?}"
        );
        assert!(
            !ledger_names().await.contains(&"903_first".to_string()),
            "validation happens up front: the harmless first file must not have run either"
        );
        reset(
            &["903_first", "904_destructive"],
            &["migration_probe_guard"],
        )
        .await;
    });
}

#[test]
fn run_refuses_a_database_older_than_the_baseline_on_first_contact() {
    common::setup_test_db();
    common::run(async {
        // First contact = empty ledger. Knock out one baseline definition to
        // stand in for a restored pre-squash backup.
        DB.query("DELETE schema_migration")
            .await
            .expect("empty ledger");
        DB.query("REMOVE FIELD IF EXISTS profile_reminders_sent ON person")
            .await
            .expect("drop probe field");
        let list = [Migration {
            name: "906_after_baseline",
            sql: "DEFINE TABLE IF NOT EXISTS migration_probe_baseline SCHEMAFULL;",
        }];

        let err = migrations::run(&list).await.expect_err("must refuse");
        assert!(
            matches!(err, MigrationError::BelowBaseline(_)),
            "got {err:?}"
        );
        assert!(
            err.to_string().contains("person.profile_reminders_sent"),
            "names what is missing: {err}"
        );
        assert!(
            ledger_names().await.is_empty(),
            "nothing may run against a below-baseline database"
        );

        // Restore the definition (as in db/schema.surql) — now the same run proceeds.
        DB.query(
            "DEFINE FIELD OVERWRITE profile_reminders_sent ON person TYPE int DEFAULT ALWAYS 0 PERMISSIONS FULL",
        )
        .await
        .expect("send")
        .check()
        .expect("restore probe field");
        let report = migrations::run(&list)
            .await
            .expect("at the baseline → runs");
        assert_eq!(report.applied, vec!["906_after_baseline"]);

        reset(&["906_after_baseline"], &["migration_probe_baseline"]).await;
    });
}

#[test]
fn baseline_probes_all_exist_in_the_shipped_schema() {
    // Guards against a probe naming something db/schema.surql no longer
    // defines, which would lock every fresh install out.
    let schema =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../db/schema.surql"))
            .expect("db/schema.surql");
    for (table, field) in migrations::BASELINE_PROBES {
        assert!(
            schema.contains(&format!("DEFINE TABLE {table} ")),
            "probe table `{table}` must be defined in db/schema.surql"
        );
        if let Some(field) = field {
            assert!(
                schema.contains(&format!("DEFINE FIELD {field} ON {table} ")),
                "probe field `{table}.{field}` must be defined in db/schema.surql"
            );
        }
    }
}

#[test]
fn run_detects_drift_without_reapplying() {
    common::setup_test_db();
    common::run(async {
        reset(&["905_drift"], &["migration_probe_drift"]).await;
        let v1 = [Migration {
            name: "905_drift",
            sql: "DEFINE TABLE IF NOT EXISTS migration_probe_drift SCHEMAFULL;",
        }];
        migrations::run(&v1).await.expect("v1 applies");

        let v2 = [Migration {
            name: "905_drift",
            sql: "DEFINE TABLE IF NOT EXISTS migration_probe_drift SCHEMAFULL;\n\
                  CREATE migration_probe_drift:x;",
        }];
        let report = migrations::run(&v2)
            .await
            .expect("drift is a warning, not an error");
        assert_eq!(report.drifted, vec!["905_drift"]);
        assert!(report.applied.is_empty());

        let mut r = DB
            .query("SELECT VALUE <string> id FROM migration_probe_drift")
            .await
            .expect("rows");
        let ids = r.take::<Vec<String>>(0).expect("take ids");
        assert!(
            ids.is_empty(),
            "the edited file must not have been executed, got {ids:?}"
        );

        reset(&["905_drift"], &["migration_probe_drift"]).await;
    });
}

#[test]
fn migration_015_applies_to_a_pre_migration_schema_and_keeps_existing_rows() {
    common::setup_test_db();
    common::clean_table("person");
    common::run(async {
        reset(&["015_profile_public_email"], &[]).await;
        // Simulate production before this deploy: the field is not defined
        // yet and a person already has a profile with data in it.
        DB.query("REMOVE FIELD IF EXISTS profile.public_email ON person")
            .await
            .expect("drop field");
        DB.query(
            "CREATE person:mig015 SET username = 'mig015', email = 'mig015@example.test', \
             password = 'h', name = 'Mig', verification_status = 'email', \
             profile = { name: 'Mig', phone: '+1 555 0100', skills: [], social_links: [], \
             ethnicity: [], unions: [], languages: [], education: [], reels: [], \
             media_other: [], awards: [] }",
        )
        .await
        .expect("send create")
        .check()
        .expect("create ok");

        let m015 = MIGRATIONS
            .iter()
            .find(|m| m.name == "015_profile_public_email")
            .copied()
            .expect("015 is shipped");
        let report = migrations::run(&[m015]).await.expect("015 applies");
        assert_eq!(report.applied, vec!["015_profile_public_email"]);

        // Existing data untouched; the new field reads as unset.
        let mut r = DB
            .query("SELECT VALUE profile.phone FROM person:mig015")
            .await
            .expect("phone");
        assert_eq!(
            r.take::<Option<String>>(0).expect("take"),
            Some("+1 555 0100".to_string())
        );
        let mut r = DB
            .query("SELECT VALUE profile.public_email FROM person:mig015")
            .await
            .expect("email");
        assert_eq!(r.take::<Option<String>>(0).expect("take"), None);

        // The field definition — with its ASSERT — is live.
        let bad = DB
            .query("UPDATE person:mig015 SET profile.public_email = 'not-an-email'")
            .await
            .expect("send")
            .check();
        assert!(bad.is_err(), "ASSERT must reject a malformed public email");
        DB.query("UPDATE person:mig015 SET profile.public_email = 'hello@example.test'")
            .await
            .expect("send")
            .check()
            .expect("valid email accepted");

        // Re-running is a no-op.
        let again = migrations::run(&[m015]).await.expect("idempotent");
        assert_eq!(again.already_applied, vec!["015_profile_public_email"]);

        let _ = DB.query("DELETE person:mig015").await;
    });
}
