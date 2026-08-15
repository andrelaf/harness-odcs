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
# Delegado: e a mesma verificacao que o pacote leva consigo. Duplicar aqui faria
# o ambiente de quem desenvolve divergir do de quem consome, que e exatamente a
# divergencia que este projeto evita em todo o resto.
"$HARNESS_ROOT/scripts/imagem.sh"

echo
echo "== build =="
cd "$HARNESS_ROOT" && cargo build --quiet
echo "  $HARNESS_BIN"

echo
echo "bootstrap concluido. Proximo: ./run.sh doctor"
