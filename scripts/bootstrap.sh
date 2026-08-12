#!/usr/bin/env sh
# Prepara o ambiente. Idempotente: rodar duas vezes da o mesmo resultado.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export HARNESS_ROOT
. "$HARNESS_ROOT/scripts/env.sh"

echo "== toolchain =="
command -v cargo >/dev/null || { echo "cargo nao encontrado — instale o Rust"; exit 1; }
cargo --version

echo
echo "== diretorios =="
for d in state trace evidence contracts; do
    mkdir -p "$HARNESS_ROOT/$d"
    echo "  $d/"
done

echo
echo "== imagem do motor de validacao =="
if docker image inspect "$DC_IMAGE" >/dev/null 2>&1; then
    echo "  $DC_IMAGE ja presente"
else
    echo "  baixando $DC_IMAGE"
    docker pull "$DC_IMAGE"
fi

atual="$(docker image inspect --format '{{index .RepoDigests 0}}' "$DC_IMAGE" 2>/dev/null || echo '')"
case "$atual" in
    *"$DC_DIGEST"*) echo "  digest confere: $DC_DIGEST" ;;
    *)
        echo "  ERRO: digest divergente"
        echo "    esperado : $DC_DIGEST"
        echo "    encontrado: ${atual:-nenhum}"
        exit 1
        ;;
esac

echo
echo "== build =="
cd "$HARNESS_ROOT" && cargo build --quiet
echo "  $HARNESS_BIN"

echo
echo "bootstrap concluido. Proximo: ./run.sh doctor"
