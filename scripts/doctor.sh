#!/usr/bin/env sh
# Despachante. As checagens vivem em src/checks.rs, num lugar so: a fase
# `smoke` e este comando chamam exatamente o mesmo codigo.
set -eu
HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
exec "$HARNESS_ROOT/run.sh" doctor "$@"
