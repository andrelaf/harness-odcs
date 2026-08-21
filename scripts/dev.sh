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
cargo clippy --workspace --all-targets -- -D warnings

echo
echo "== testes =="
# `--workspace` nao e enfeite. A raiz do workspace tambem e um pacote — o
# binario —, e `cargo test` sem a flag roda **so ele**: zero testes, saida
# verde. O guarda do release cairia nisso em silencio.
cargo test --workspace
