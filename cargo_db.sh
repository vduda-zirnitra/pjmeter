#!/usr/bin/env bash
set -euo pipefail

# Project root = where this script lives
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Target dir honors CARGO_TARGET_DIR, else ./target
TARGET_DIR="${CARGO_TARGET_DIR:-"$PROJECT_ROOT/target"}"
SQLX_DIR="$TARGET_DIR/sqlx"
COMBINED_DIR="$SQLX_DIR/combined_migrations"
DB_PATH="$SQLX_DIR/build-check.db"

print_usage() {
  cat <<'USAGE'
Usage:
  ./cargo_db.sh build [cargo build options...]
  ./cargo_db.sh run   [cargo run options...]
  ./cargo_db.sh test  [cargo test options...]

Examples:
  ./cargo_db.sh build --release
  ./cargo_db.sh run -- --flag value
  ./cargo_db.sh run --release -- --flag value
  ./cargo_db.sh test
  ./cargo_db.sh test --release

Notes:
- Rebuilds a fresh temp DB at target/sqlx/build-check.db every time.
- Combines migrations from multiple directories into target/sqlx/combined_migrations
  with unique sequential versions so sqlx sees one coherent history.
- Override sources with env var (space-separated paths, relative to repo root allowed):
    MIGRATIONS_DIRS="database_1 database_2" ./cargo_db.sh build
  If unset, the script will look for any of these that exist:
    ./migrations, ./migrations_local, ./migrations_global, ./database_1, ./database_2
USAGE
}

ensure_sqlx_cli() {
  if ! command -v sqlx >/dev/null 2>&1; then
    echo "Error: sqlx CLI not found. Install with:
  cargo install sqlx-cli --no-default-features --features sqlite" >&2
    exit 1
  fi
}

# Gather .sql files in lexicographic order (standard sqlx layout is flat files).
gather_sql_files() {
  local src="$1"
  # Flat files at top level
  if compgen -G "$src/*.sql" > /dev/null; then
    find "$src" -maxdepth 1 -type f -name '*.sql' | sort
    return
  fi
  # No .sql found
  return 0
}

build_combined_migrations() {
  rm -rf "$COMBINED_DIR"
  mkdir -p "$COMBINED_DIR"

  # Resolve sources
  declare -a SOURCES=()
  if [[ -n "${MIGRATIONS_DIRS:-}" ]]; then
    read -r -a SOURCES <<<"$MIGRATIONS_DIRS"
    for i in "${!SOURCES[@]}"; do
      [[ "${SOURCES[$i]}" = /* ]] || SOURCES[$i]="$PROJECT_ROOT/${SOURCES[$i]}"
    done
  else
    for d in \
      "$PROJECT_ROOT/migrations/pjmeter_settings" \
      "$PROJECT_ROOT/migrations/projects_settings"
    do
      [[ -d "$d" ]] && SOURCES+=("$d")
    done
  fi

  if [[ ${#SOURCES[@]} -eq 0 ]]; then
    echo "Error: No migration directories found. Set MIGRATIONS_DIRS or create one of the default dirs." >&2
    exit 1
  fi

  local idx=1
  for src in "${SOURCES[@]}"; do
    mapfile -t files < <(gather_sql_files "$src" || true)
    if [[ ${#files[@]} -eq 0 ]]; then
      echo "Warning: no .sql files in $src; skipping" >&2
      continue
    fi
    for f in "${files[@]}"; do
      local n; n=$(printf "%06d" "$idx")
      local base; base="$(basename "$f")"
      # Keep a trace of origin in filename; only the leading number matters to sqlx.
      local out="$COMBINED_DIR/${n}__from_$(basename "$src")__${base}"
      cp "$f" "$out"
      ((idx++))
    done
  done

  if ! find "$COMBINED_DIR" -type f -name '*.sql' -print -quit >/dev/null; then
    echo "Error: Combined migrations directory is empty." >&2
    exit 1
  fi

  echo "Combined migrations written to: $COMBINED_DIR"
}

make_sqlite_url() {
  local p="$1"
  # ensure absolute
  if [[ "$p" != /* ]]; then
    p="$(cd "$(dirname "$p")" && pwd)/$(basename "$p")"
  fi
  printf 'sqlite:///%s' "${p#/}"
}

prepare_temp_db() {
  mkdir -p "$SQLX_DIR"
  rm -f "$DB_PATH"

  local db_url; db_url="$(make_sqlite_url "$DB_PATH")"

  # Create empty DB and apply the combined migrations once
  sqlx database create --database-url "$db_url" >/dev/null
  sqlx migrate run --source "$COMBINED_DIR" --database-url "$db_url" >/dev/null

  # IMPORTANT: print ONLY the URL to stdout so command substitution captures just this.
  printf '%s' "$db_url"
}

main() {
  if [[ $# -lt 1 ]]; then
    print_usage
    exit 1
  fi

  local cmd="$1"; shift || true
  case "$cmd" in
    build|run|test) ;;
    -h|--help) print_usage; exit 0 ;;
    *) echo "Error: unknown command '$cmd' (expected 'build', 'run', or 'test')" >&2; print_usage; exit 1 ;;
  esac

  ensure_sqlx_cli
  build_combined_migrations
  db_url="$(prepare_temp_db)"

  DATABASE_URL=$db_url cargo "$cmd" "$@"
}

main "$@"