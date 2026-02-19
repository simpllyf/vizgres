version := `cat VERSION`
msrv := "1.93"

# ─── Development ───────────────────────────────────────

# Format code
fmt:
    cargo fmt --all

# Run lints (format check + clippy)
lint:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings

# Run unit and doc tests
test:
    cargo test --lib
    cargo test --doc

# Run integration tests (requires running PostgreSQL)
test-integration:
    cargo test --test integration

# Check compilation
check:
    cargo check

# Check MSRV compilation
check-msrv:
    cargo +{{msrv}} check

# Build debug
build:
    cargo build --all-features

# Build release
build-release:
    cargo build --release --all-features

# Generate code coverage
coverage:
    cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

# Start test database
db-up:
    docker compose -f docker-compose.test.yml up -d --wait

# Stop test database
db-down:
    docker compose -f docker-compose.test.yml down

# Regenerate data/sql_keywords.txt from PostgreSQL docs
update-keywords:
    curl -s https://www.postgresql.org/docs/18/sql-keywords-appendix.html \
        | grep -oP '<code class="token">[A-Z_]+</code>' \
        | sed 's/<[^>]*>//g' \
        | sort -u \
        | awk 'length > 1' \
        > data/sql_keywords.txt
    @echo "Updated data/sql_keywords.txt ($(wc -l < data/sql_keywords.txt) keywords)"

# ─── Version ──────────────────────────────────────────

# Stamp VERSION into Cargo.toml and regenerate lockfile
stamp-version:
    sed -i 's/^version = "[^"]*"/version = "{{version}}"/' Cargo.toml
    cargo generate-lockfile

# Print current version
versions:
    @echo "vizgres=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "vizgres") | .version')"

# ─── Release ──────────────────────────────────────────

# Create a release PR: just release 1.2.3
release new_version:
    #!/usr/bin/env bash
    set -euo pipefail

    if ! echo "{{new_version}}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "Error: '{{new_version}}' is not valid semver (expected X.Y.Z)"
        exit 1
    fi

    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: working tree is not clean"
        exit 1
    fi

    branch=$(git branch --show-current)
    if [ "$branch" != "main" ]; then
        echo "Error: must be on main (currently on '$branch')"
        exit 1
    fi

    git pull --ff-only

    echo "{{new_version}}" > VERSION
    just version="{{new_version}}" stamp-version

    git checkout -b "release/v{{new_version}}"
    git add .
    git commit -m "release: v{{new_version}}"
    git push -u origin "release/v{{new_version}}"
    gh pr create --title "release: v{{new_version}}" --body "Bump version to {{new_version}} and publish."
    echo "Release PR created for v{{new_version}}"

# Publish crate to crates.io (local fallback)
publish-crate:
    cargo publish
