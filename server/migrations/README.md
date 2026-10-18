# Schema migrations

Incremental SurrealQL, applied **automatically by the server at boot**
(`src/db/migrations.rs`) in file-name order, each file exactly once. The
ledger of applied files is the `schema_migration` table.

`db/schema.surql` is the full schema for fresh installs and the squash
baseline: migrations 001–014 were folded into it on 2026-10-18 and exist only
in git history (`git log -- db/migrations`). Numbering continues from 015 here.

## The rules

1. **Never remove data.** No migration may drop tables, fields, databases or
   namespaces, `DELETE` rows, or `UNSET`/null out field values unless Chris
   Bruce has explicitly authorized that specific change. The runner enforces
   this mechanically: a file containing `REMOVE TABLE|FIELD|DATABASE|NAMESPACE`,
   `DELETE` or `UNSET` is refused at boot (and by `make test`) unless it
   carries a line of the form

       -- destructive: authorized by Chris Bruce, 2026-10-18 — <what and why>

   `--` comments are ignored by the scan, so mentioning those words in prose
   is fine. Renames are done additively: add the new field, backfill it,
   leave the old one in place.

2. **Idempotent.** Use `DEFINE … OVERWRITE` / `DEFINE … IF NOT EXISTS` and
   guard backfills with `WHERE field IS NONE`. A fresh install imports the
   full `db/schema.surql` and then still runs every migration.

3. **Mirror into `db/schema.surql`.** The schema file is the single source of
   truth for fresh installs; every migration's end state must also be in it.

4. **New non-optional fields on existing tables** must be `DEFAULT ALWAYS x`
   plus a backfill `UPDATE tbl SET f = x WHERE f IS NONE;` — or be `option<>`.
   A plain `DEFAULT` leaves legacy rows NONE and then breaks every later
   UPDATE to them, because SCHEMAFULL re-validates the whole row.

5. **One transaction per file.** The runner wraps each file in
   `BEGIN TRANSACTION … COMMIT TRANSACTION` together with its ledger row, so
   do not add your own. End every statement with `;`. If a statement fails,
   the whole file rolls back and the server refuses to start — fix forward
   and redeploy.

6. **Applied files are immutable.** The ledger stores a checksum; editing a
   file after it has run on production logs a drift warning at boot and the
   file is skipped, never re-applied. Write a new file instead.

## Adding a migration

1. Create `NNN_snake_case_name.surql` with the next number.
2. Register it in `MIGRATIONS` in `src/db/migrations.rs` (`include_str!`).
   `tests/migrations_test.rs` fails if the directory and the list disagree.
3. Mirror the change into `db/schema.surql`.
4. `make test` — the file is validated and exercised against the test DB.
5. Deploy. The boot log reports `applying schema migration` for each new file.

Manual application to a database the server is *not* running against:
`make db-migrate MIGRATION=015_profile_public_email`. This bypasses the
ledger; the server will still treat the file as pending and run it again,
which is harmless because migrations are idempotent.
