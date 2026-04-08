#!/usr/bin/env bash
# Download and load IMDb non-commercial datasets into the local PostgreSQL container.
# Works with both Docker and Podman. No local psql required — runs psql inside the container.
#
# Usage: ./scripts/load-imdb.sh
#
# Datasets: https://developer.imdb.com/non-commercial-datasets/
# License:  Personal and non-commercial use only.
#
# Environment overrides:
#   IMDB_DATA_DIR  — where to download/cache TSV files (default: .imdb-data/)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DATA_DIR="${IMDB_DATA_DIR:-$PROJECT_DIR/.imdb-data}"
SCHEMA_FILE="$SCRIPT_DIR/imdb-schema.sql"

DB_NAME="imdb"
DB_USER="test_user"
DB_PASS="test_password"
CONTAINER="vizgres-test-db"

BASE_URL="https://datasets.imdbws.com"
DATASETS=(
    "title.basics"
    "title.akas"
    "title.crew"
    "title.episode"
    "title.principals"
    "title.ratings"
    "name.basics"
)

declare -A TABLE_NAMES=(
    ["title.basics"]="title_basics"
    ["title.akas"]="title_akas"
    ["title.crew"]="title_crew"
    ["title.episode"]="title_episode"
    ["title.principals"]="title_principals"
    ["title.ratings"]="title_ratings"
    ["name.basics"]="name_basics"
)

declare -A TABLE_COLUMNS=(
    ["title.basics"]="tconst,title_type,primary_title,original_title,is_adult,start_year,end_year,runtime_minutes,genres"
    ["title.akas"]="title_id,ordering,title,region,language,types,attributes,is_original"
    ["title.crew"]="tconst,directors,writers"
    ["title.episode"]="tconst,parent_tconst,season_number,episode_number"
    ["title.principals"]="tconst,ordering,nconst,category,job,characters"
    ["title.ratings"]="tconst,average_rating,num_votes"
    ["name.basics"]="nconst,primary_name,birth_year,death_year,primary_profession,known_for_titles"
)

info()  { echo "==> $*"; }
error() { echo "ERROR: $*" >&2; exit 1; }

# ── Detect container runtime ───────────────────────────────────
detect_runtime() {
    if command -v docker &>/dev/null; then
        echo "docker"
    elif command -v podman &>/dev/null; then
        echo "podman"
    else
        error "Neither docker nor podman found in PATH"
    fi
}

RUNTIME="$(detect_runtime)"

# Run psql inside the container — no local psql dependency
container_psql() {
    "$RUNTIME" exec -i "$CONTAINER" env PGPASSWORD="$DB_PASS" psql -U "$DB_USER" "$@"
}

# ── Ensure PostgreSQL container is running ──────────────────────
ensure_postgres() {
    if "$RUNTIME" ps --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
        info "PostgreSQL container '$CONTAINER' is running ($RUNTIME)"
    else
        info "Starting PostgreSQL via $RUNTIME compose..."
        "$RUNTIME" compose -f "$PROJECT_DIR/docker-compose.test.yml" up -d --wait
    fi
}

# ── Create imdb database if needed ──────────────────────────────
ensure_database() {
    if container_psql -d postgres -tAc \
        "SELECT 1 FROM pg_database WHERE datname = '$DB_NAME'" | grep -q 1; then
        info "Database '$DB_NAME' exists"
    else
        info "Creating database '$DB_NAME'..."
        container_psql -d postgres -c "CREATE DATABASE $DB_NAME"
    fi
}

# ── Download datasets ───────────────────────────────────────────
download_datasets() {
    mkdir -p "$DATA_DIR"

    for dataset in "${DATASETS[@]}"; do
        local file="$dataset.tsv.gz"
        local url="$BASE_URL/$file"
        local target="$DATA_DIR/$file"

        if [[ -f "$target" ]]; then
            # stat -c on Linux, stat -f on macOS
            local mtime
            mtime=$(stat -c %Y "$target" 2>/dev/null || stat -f %m "$target")
            local age_hours=$(( ($(date +%s) - mtime) / 3600 ))
            if [[ $age_hours -lt 24 ]]; then
                info "Skipping $file (downloaded ${age_hours}h ago)"
                continue
            fi
        fi

        info "Downloading $file..."
        curl -fL --progress-bar -o "$target" "$url"
    done
}

# ── Load a single dataset ──────────────────────────────────────
load_dataset() {
    local dataset="$1"
    local table="${TABLE_NAMES[$dataset]}"
    local columns="${TABLE_COLUMNS[$dataset]}"
    local file="$DATA_DIR/$dataset.tsv.gz"

    info "Loading $dataset -> $table..."

    container_psql -d "$DB_NAME" -c "TRUNCATE $table"

    # Stream gunzipped TSV (skip header) into container's psql via COPY.
    # QUOTE set to 0x01 (SOH) to disable CSV quoting — IMDb fields contain
    # unescaped double quotes but never use CSV-style quoting.
    gunzip -c "$file" \
        | tail -n +2 \
        | container_psql -d "$DB_NAME" \
            -c "COPY $table ($columns) FROM STDIN WITH (FORMAT csv, DELIMITER E'\\t', NULL '\\N', QUOTE E'\\x01')"

    local count
    count=$(container_psql -d "$DB_NAME" -tAc \
        "SELECT to_char(count(*), 'FM999,999,999') FROM $table")
    info "  -> $table: $count rows"
}

# ── Create indexes for realistic query performance ──────────────
create_indexes() {
    info "Creating indexes..."
    container_psql -d "$DB_NAME" <<'SQL'
CREATE INDEX IF NOT EXISTS idx_basics_type ON title_basics (title_type);
CREATE INDEX IF NOT EXISTS idx_basics_start_year ON title_basics (start_year);
CREATE INDEX IF NOT EXISTS idx_basics_primary_title ON title_basics (primary_title);
CREATE INDEX IF NOT EXISTS idx_akas_title_id ON title_akas (title_id);
CREATE INDEX IF NOT EXISTS idx_episode_parent ON title_episode (parent_tconst);
CREATE INDEX IF NOT EXISTS idx_principals_nconst ON title_principals (nconst);
CREATE INDEX IF NOT EXISTS idx_ratings_votes ON title_ratings (num_votes DESC);
CREATE INDEX IF NOT EXISTS idx_names_name ON name_basics (primary_name);
SQL
    info "Indexes created"
}

# ── Main ────────────────────────────────────────────────────────
main() {
    info "IMDb dataset loader for vizgres"
    echo ""

    ensure_postgres
    ensure_database

    info "Applying schema..."
    container_psql -d "$DB_NAME" < "$SCHEMA_FILE"

    download_datasets

    echo ""
    info "Loading datasets into PostgreSQL..."
    for dataset in "${DATASETS[@]}"; do
        load_dataset "$dataset"
    done

    echo ""
    create_indexes

    info "Running ANALYZE..."
    container_psql -d "$DB_NAME" -c "ANALYZE"

    echo ""
    info "Done! Connect with:"
    echo "  vizgres postgres://$DB_USER:$DB_PASS@localhost:5433/$DB_NAME"
}

main "$@"
