#!/usr/bin/env sh
# Wrapper do datacontract-cli em container, com o mount padrao.
#
# Uso: ./scripts/datacontract.sh lint contracts/clientes.odcs.yaml
#
# Existe para que a versao da imagem e o formato do mount fiquem num lugar so.
# O caminho do host vai em formato nativo (HARNESS_ROOT_NATIVE) porque o Git
# Bash reescreveria um caminho POSIX e o container subiria com o diretorio
# vazio — falha silenciosa e cara de diagnosticar.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export HARNESS_ROOT
. "$HARNESS_ROOT/scripts/env.sh"

exec env MSYS_NO_PATHCONV=1 docker run --rm \
    -v "$HARNESS_ROOT_NATIVE:/home/datacontract" \
    "$DC_IMAGE" "$@"
