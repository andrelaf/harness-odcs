#!/usr/bin/env sh
# Roda a mesma verificacao que o pull request vai rodar.
#
#   ./scripts/verificar.sh                                  # resolve o contrato sozinho
#   ./scripts/verificar.sh --contrato contracts/clientes/cadastro/contract.odcs.yaml
#   ./scripts/verificar.sh --formato markdown               # o comentario que o PR receberia
#
# Exit code e o veredito:
#
#   0  passou
#   1  reprovou — o motivo esta na saida, ancorado no arquivo
#   5  bloqueado, aguardando decisao humana — nao e erro seu
#
# Nao escreve no contrato. O contrato enriquecido e o laudo saem propostos em
# `evidence/`, e entram no repositorio quando alguem aceitar a proposta.
set -eu

RAIZ="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$RAIZ"

[ -x "$RAIZ/.harness/harness.sh" ] || {
    echo "pacote de validacao ausente — rode ./scripts/preparar.sh" >&2
    exit 2
}

# A imagem do motor de validacao, no digest fixado. Idempotente.
"$RAIZ/.harness/scripts/imagem.sh"

exec "$RAIZ/.harness/harness.sh" check "$@"
