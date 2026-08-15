#!/usr/bin/env sh
# O que o CI executa — e exatamente o que voce roda na sua maquina.
#
# Um comando so, tres contextos: Claude Code, VS Code e GitHub Actions. Se o CI
# tivesse a sua propria sequencia de passos, ela seria uma segunda politica e
# divergiria da local na primeira semana.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

"$HARNESS_ROOT/scripts/bootstrap.sh"
echo
"$HARNESS_ROOT/scripts/dev.sh"
echo
"$HARNESS_ROOT/run.sh" doctor
