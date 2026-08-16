#!/usr/bin/env sh
# Baixa o pacote de validacao fixado em `harness.lock` e o deixa em `.harness/`.
#
# Existe para que a verificacao local seja a **mesma** do pipeline, e nao uma
# aproximacao. Le o mesmo arquivo de pin, confere o mesmo sha256, extrai o mesmo
# tarball. A unica diferenca entre esta maquina e o runner e o sistema
# operacional do binario.
#
#   ./scripts/preparar.sh      # baixa (idempotente)
#   ./scripts/verificar.sh     # roda o check
#
# Usa `gh` quando existir — e o que vai continuar funcionando no dia em que o
# repositorio do harness for privado — e cai para `curl` quando nao existir,
# que basta enquanto ele for publico.
set -eu

RAIZ="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$RAIZ"

# shellcheck disable=SC1091
. ./harness.lock

DESTINO="$RAIZ/.harness"
TAR="$RAIZ/.harness-download/harness-odcs-linux-x64.tar.gz"

# Ja preparado na versao certa? Sai sem fazer nada. Rodar antes de cada
# verificacao nao pode custar um download.
if [ -f "$DESTINO/VERSION" ] && grep -q "$HARNESS_VERSAO" "$DESTINO/VERSION" 2>/dev/null; then
    echo "pacote $HARNESS_VERSAO ja preparado em .harness/"
    exit 0
fi

echo "== baixando $HARNESS_REPO@$HARNESS_VERSAO =="
mkdir -p "$(dirname "$TAR")"
rm -f "$TAR"
if command -v gh >/dev/null 2>&1; then
    gh release download "$HARNESS_VERSAO" \
        --repo "$HARNESS_REPO" \
        --pattern 'harness-odcs-linux-x64.tar.gz' \
        --dir "$(dirname "$TAR")"
else
    curl -fsSL -o "$TAR" \
        "https://github.com/$HARNESS_REPO/releases/download/$HARNESS_VERSAO/harness-odcs-linux-x64.tar.gz"
fi

echo "== conferindo o sha256 =="
echo "$HARNESS_SHA256  $TAR" | sha256sum --check --strict

echo "== extraindo =="
rm -rf "$DESTINO"
mkdir -p "$RAIZ/.harness-tmp"
tar -C "$RAIZ/.harness-tmp" -xzf "$TAR"
mv "$RAIZ/.harness-tmp/harness-odcs" "$DESTINO"
rm -rf "$RAIZ/.harness-tmp"

echo
cat "$DESTINO/VERSION"
echo
echo "pronto. Proximo: ./scripts/verificar.sh"
