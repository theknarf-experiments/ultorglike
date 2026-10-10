#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
if [ "${1:-}" = "--postgres" ]; then
  docker compose up -d --wait
  export TEST_DATABASE_URL="${TEST_DATABASE_URL:-postgresql://ultorg:ultorg@localhost:${POSTGRES_PORT:-55432}/ultorg}"
  cargo test -p query-engine --test postgres --locked -- --ignored --test-threads=1
fi
