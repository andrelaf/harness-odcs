#!/usr/bin/env sh
# Baixa os **dois** pacotes fixados em `harness.lock`: a ferramenta em
# `.harness/` e o vocabulario em `.harness-vocab/`.
#
# Existe para que a verificacao local seja a **mesma** do pipeline, e nao uma
# aproximacao. Le o mesmo arquivo de pin, confere os mesmos sha256, extrai os
# mesmos tarballs. A unica diferenca entre esta maquina e o runner e o sistema
# operacional do binario.
#
#   ./scripts/preparar.sh      # baixa (idempotente)
#   ./scripts/verificar.sh     # roda o check
#
# --- Por que sao dois
#
# Porque respondem a duas perguntas de auditoria diferentes — "que codigo
# julgou?" e "sob qual vocabulario?" — e mudam em cadencias diferentes. Com um
# pacote so, cadastrar um termo exigia publicar um binario novo, e o dono do
# vocabulario ficava na fila do dono do codigo.
#
# O binario e preso a plataforma (`linux-x64`); o vocabulario e YAML e nao e.
# Por isso o segundo download funciona em qualquer maquina, inclusive onde o
# primeiro nao serve.
#
# Usa `gh` quando existir — e o que vai continuar funcionando no dia em que o
# repositorio do harness for privado — e cai para `curl` quando nao existir,
# que basta enquanto ele for publico.
set -eu

RAIZ="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$RAIZ"

# shellcheck disable=SC1091
. ./harness.lock

BAIXADOS="$RAIZ/.harness-download"

# Baixa, confere o sha256 e extrai. Um so lugar para os dois pacotes: duas
# copias deste procedimento divergiriam na primeira vez que um deles ganhasse
# uma verificacao a mais.
#
#   $1 repo  $2 versao  $3 sha256  $4 nome do asset
#   $5 diretorio dentro do tarball  $6 destino final
baixar() {
    repo="$1"; versao="$2"; sha="$3"; asset="$4"; dentro="$5"; destino="$6"
    tar_local="$BAIXADOS/$asset"

    # Ja preparado na versao certa? Sai sem fazer nada. Rodar antes de cada
    # verificacao nao pode custar um download.
    if [ -f "$destino/VERSION" ] && grep -q "$versao" "$destino/VERSION" 2>/dev/null; then
        echo "== $asset: $versao ja preparado em $(basename "$destino")/ =="
        return 0
    fi

    echo "== baixando $repo@$versao ($asset) =="
    mkdir -p "$BAIXADOS"
    rm -f "$tar_local"
    if command -v gh >/dev/null 2>&1; then
        gh release download "$versao" \
            --repo "$repo" \
            --pattern "$asset" \
            --dir "$BAIXADOS"
    else
        curl -fsSL -o "$tar_local" \
            "https://github.com/$repo/releases/download/$versao/$asset"
    fi

    echo "== conferindo o sha256 =="
    echo "$sha  $tar_local" | sha256sum --check --strict

    echo "== extraindo =="
    rm -rf "$destino"
    tmp="$RAIZ/.harness-tmp"
    rm -rf "$tmp"
    mkdir -p "$tmp"
    tar -C "$tmp" -xzf "$tar_local"
    mv "$tmp/$dentro" "$destino"
    rm -rf "$tmp"

    echo
    cat "$destino/VERSION"
    echo
}

baixar "$HARNESS_REPO" "$HARNESS_VERSAO" "$HARNESS_SHA256" \
    "harness-odcs-linux-x64.tar.gz" "harness-odcs" "$RAIZ/.harness"

baixar "$HARNESS_VOCAB_REPO" "$HARNESS_VOCAB_VERSAO" "$HARNESS_VOCAB_SHA256" \
    "harness-vocab.tar.gz" "harness-vocab" "$RAIZ/.harness-vocab"

echo "pronto. Proximo: ./scripts/verificar.sh"
