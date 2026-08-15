#!/usr/bin/env sh
# Garante a imagem do motor de validacao, no digest fixado.
#
# Separado do `bootstrap.sh` porque os dois publicos sao diferentes: quem
# desenvolve o harness precisa de cargo, diretorios e build; quem apenas
# **usa** o pacote contra os proprios contratos nao precisa de nada disso — mas
# precisa da imagem, e precisa dela no digest certo.
#
# Vai dentro do pacote. Sem ele, o repositorio de contratos teria de repetir a
# verificacao de digest no YAML do pipeline, e a garantia de reprodutibilidade
# passaria a existir em dois lugares que divergem.
set -eu

HARNESS_HOME="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
: "${HARNESS_ROOT:=$HARNESS_HOME}"
export HARNESS_HOME HARNESS_ROOT
. "$HARNESS_HOME/scripts/env.sh"

if docker image inspect "$DC_IMAGE" >/dev/null 2>&1; then
    echo "  $DC_IMAGE ja presente"
else
    echo "  baixando $DC_IMAGE"
    docker pull "$DC_IMAGE"
fi

# `image inspect` responde com o digest do que **esta** na maquina. Comparar com
# o fixado e o que impede a mesma tag de significar duas coisas em datas
# diferentes — o contrato nao pode passar hoje e reprovar amanha sem commit.
atual="$(docker image inspect --format '{{index .RepoDigests 0}}' "$DC_IMAGE" 2>/dev/null || echo '')"
case "$atual" in
    *"$DC_DIGEST"*)
        echo "  digest confere: $DC_DIGEST"
        ;;
    *)
        echo "  ERRO: digest divergente" >&2
        echo "    esperado  : $DC_DIGEST" >&2
        echo "    encontrado: ${atual:-nenhum}" >&2
        exit 1
        ;;
esac
