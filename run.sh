#!/usr/bin/env sh
# Ponto de entrada unico do harness.
#
# Este arquivo e um despachante: resolve a raiz, carrega o ambiente, garante o
# binario compilado e repassa argv. Nao ha regra de fluxo aqui — ela vive em
# src/flow.rs, compilada. Se este script ganhar um `if` de negocio, a politica
# passou a existir em dois lugares e a portabilidade entre IDEs quebra.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
export HARNESS_ROOT

. "$HARNESS_ROOT/scripts/env.sh"

cd "$HARNESS_ROOT"
cargo build --quiet

exec "$HARNESS_BIN" "$@"
