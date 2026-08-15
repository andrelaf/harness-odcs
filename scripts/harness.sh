#!/usr/bin/env sh
# Ponto de entrada do harness **empacotado** — o que roda fora deste
# repositorio.
#
# Vai para a raiz do pacote extraido (o release), ao lado de `bin/`,
# `scripts/`, `glossary/` e `classification/`. O `run.sh` continua sendo o
# ponto de entrada de quem desenvolve o harness; este e o de quem apenas o
# **usa** contra os proprios contratos.
#
# A unica diferenca entre os dois: `run.sh` compila, este nao. Um repositorio
# de contratos nao tem `src/`, nao tem cargo e nao deveria precisar de nenhum
# dos dois para saber se o contrato passa.
#
#   HARNESS_HOME   este diretorio  — o binario e o vocabulario vem daqui
#   HARNESS_ROOT   o diretorio de trabalho — os contratos, o trace, a evidencia
#
# Uso, de dentro do repositorio de contratos:
#
#   /caminho/do/pacote/harness.sh check --formato github
#
set -eu

# A ferramenta esta onde este script esta. Resolvido por `dirname`, e nao por
# variavel de ambiente, para que o pacote funcione extraido em qualquer lugar.
HARNESS_HOME="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

# Os dados sao o diretorio de trabalho de quem chamou. E a inversao que importa:
# aqui a ferramenta e que visita o repositorio, e nao o contrario.
: "${HARNESS_ROOT:=$(pwd)}"

# O criterio vem do pacote. Apontar para outro lugar e o que permite fixar o
# vocabulario numa versao propria, independente da versao do binario — ver
# `.github/README.md` no repositorio de contratos.
: "${HARNESS_VOCAB:=$HARNESS_HOME}"

export HARNESS_HOME HARNESS_ROOT HARNESS_VOCAB

. "$HARNESS_HOME/scripts/env.sh"

if [ ! -x "$HARNESS_BIN" ]; then
    echo "binario nao encontrado em $HARNESS_BIN — o pacote esta incompleto" >&2
    exit 2
fi

# O binario resolve tudo por caminho absoluto a partir de HARNESS_ROOT, mas o
# `docker run` monta o diretorio de trabalho: sem este `cd`, o mount aponta
# para o pacote em vez do repositorio.
cd "$HARNESS_ROOT"
exec "$HARNESS_BIN" "$@"
