#!/usr/bin/env bash
# Database migration runner for SurrealDB.
#
# Finds all db/migrations/NNNN_*.surql files, checks each against the
# schema_migration table, and applies any that haven't been run yet.
#
# Usage: bash db/migrate.sh <DB_USER> <DB_PASS> <NAMESPACE> <DATABASE>

set -euo pipefail

DB_USER="${1:?Usage: migrate.sh <DB_USER> <DB_PASS> <NAMESPACE> <DATABASE>}"
DB_PASS="${2:?}"
NAMESPACE="${3:?}"
DATABASE="${4:?}"
ENDPOINT="http://localhost:8000"
CONTAINER="slatehub-surrealdb"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MIGRATIONS_DIR="$SCRIPT_DIR/migrations"

surreal_sql() {
    docker exec -i "$CONTAINER" /surreal sql \
        --endpoint "$ENDPOINT" \
        --username "$DB_USER" \
        --password "$DB_PASS" \
        --namespace "$NAMESPACE" \
        --database "$DATABASE" \
        --hide-welcome
}

# Collect migration files sorted by name
shopt -s nullglob
FILES=("$MIGRATIONS_DIR"/[0-9]*_*.surql)
shopt -u nullglob

if [ ${#FILES[@]} -eq 0 ]; then
    echo "No migration files found in $MIGRATIONS_DIR"
    exit 0
fi

applied=0
skipped=0
failed=0

for file in "${FILES[@]}"; do
    name="$(basename "$file" .surql)"

    # Check if already applied — query returns the count
    count=$(echo "SELECT VALUE count() FROM schema_migration WHERE name = '$name' GROUP ALL;" \
        | surreal_sql 2>/dev/null \
        | tr -d '[:space:][]"' || echo "0")

    # If we got a number > 0, skip
    if [[ "$count" =~ ^[0-9]+$ ]] && [ "$count" -gt 0 ]; then
        echo "  skip: $name (already applied)"
        skipped=$((skipped + 1))
        continue
    fi

    echo "  apply: $name ..."

    # Run the migration
    if cat "$file" | surreal_sql > /dev/null 2>&1; then
        # Record it as applied
        echo "INSERT INTO schema_migration (name) VALUES ('$name');" \
            | surreal_sql > /dev/null 2>&1
        echo "    ✅ $name applied"
        applied=$((applied + 1))
    else
        echo "    ❌ $name FAILED"
        failed=$((failed + 1))
        # Stop on first failure — don't skip ahead
        echo ""
        echo "Migration failed. Fix the issue and re-run."
        exit 1
    fi
done

echo ""
echo "Migrations complete: $applied applied, $skipped skipped, $failed failed"
