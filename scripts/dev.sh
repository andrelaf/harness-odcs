#!/usr/bin/env sh
# Loop de desenvolvimento: formato, lint e testes.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export HARNESS_ROOT
. "$HARNESS_ROOT/scripts/env.sh"

cd "$HARNESS_ROOT"

echo "== fmt =="
cargo fmt --all

echo
echo "== clippy =="
cargo clippy --all-targets -- -D warnings

echo
echo "== testes =="
cargo test
