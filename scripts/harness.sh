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

# O criterio **nao** vem mais do pacote: ele tem release e pin proprios, para
# que cadastrar um termo nao exija publicar um binario. Quem chama aponta
# `HARNESS_VOCAB` para o vocabulario que fixou no `harness.lock`.
#
# O default continua sendo este diretorio, e nao e legado: e o que faz o
# repositorio do harness — onde o vocabulario mora ao lado do codigo — nao
# precisar exportar nada para operar.
: "${HARNESS_VOCAB:=$HARNESS_HOME}"

export HARNESS_HOME HARNESS_ROOT HARNESS_VOCAB

. "$HARNESS_HOME/scripts/env.sh"

if [ ! -x "$HARNESS_BIN" ]; then
    echo "binario nao encontrado em $HARNESS_BIN — o pacote esta incompleto" >&2
    exit 2
fi

# Falha aqui, e nao la dentro. Sem esta checagem o binario reclamaria de
# "glossario nao encontrado" num caminho que quem chamou nunca escolheu — o
# default —, e a mensagem mandaria procurar um arquivo em vez de mandar apontar
# a variavel. Diagnostico errado sobrevive a tarde inteira.
if [ ! -f "$HARNESS_VOCAB/glossary/glossario.yaml" ]; then
    echo "vocabulario nao encontrado em $HARNESS_VOCAB" >&2
    echo >&2
    echo "  O pacote do binario nao carrega mais glossario e catalogo: eles tem" >&2
    echo "  release proprio, para que cadastrar um termo nao exija publicar um" >&2
    echo "  binario. Baixe o pacote fixado em HARNESS_VOCAB_VERSAO e aponte:" >&2
    echo >&2
    echo "    HARNESS_VOCAB=/caminho/do/harness-vocab $0 $*" >&2
    exit 2
fi

# O binario resolve tudo por caminho absoluto a partir de HARNESS_ROOT, mas o
# `docker run` monta o diretorio de trabalho: sem este `cd`, o mount aponta
# para o pacote em vez do repositorio.
cd "$HARNESS_ROOT"
exec "$HARNESS_BIN" "$@"
