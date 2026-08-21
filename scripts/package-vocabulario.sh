#!/usr/bin/env sh
# Monta o pacote distribuivel do **vocabulario** — glossario e catalogo.
#
# Irmao de `package.sh`, e separado dele pelo mesmo motivo que os dois releases
# sao separados: o criterio muda numa cadencia diferente do codigo que o aplica.
# Cadastrar `segmento` nao deveria exigir compilar Rust.
#
#   ./scripts/package-vocabulario.sh [destino]
#
# Produz:
#
#   <destino>/
#     glossary/glossario.yaml        o que cada termo significa
#     classification/catalogo-lgpd.yaml   o que a lei diz sobre cada termo
#     VERSION                        procedencia: versao e sha256 de cada um
#
# --- Por que a VERSION lista os dois separadamente
#
# Porque sao dois donos com cadencias diferentes — o glossario e do data
# steward, o catalogo e do encarregado de dados — e cada um tem sua `version`.
# Uma revisao juridica que corrige uma justificativa move o catalogo e nao o
# glossario, e a pergunta de auditoria "qual catalogo classificou este campo?"
# precisa de resposta propria.
#
# O sha256 de cada arquivo entra junto porque `version` e uma declaracao, e o
# hash e um fato: os dois divergirem e exatamente o defeito que o CI procura.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
DESTINO="${1:-$HARNESS_ROOT/dist/harness-vocab}"

# `sha256sum` no Linux e no git bash; `shasum -a 256` no macOS. Mesma escolha
# do `package.sh`, e pelo mesmo motivo: o pacote precisa ser montavel na
# maquina de quem edita o vocabulario, que nao e necessariamente Linux.
sha256_de() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

versao_de() {
    grep -m1 '^version:' "$1" | sed 's/^version:[[:space:]]*//'
}

GLOSSARIO="$HARNESS_ROOT/glossary/glossario.yaml"
CATALOGO="$HARNESS_ROOT/classification/catalogo-lgpd.yaml"

for arquivo in "$GLOSSARIO" "$CATALOGO"; do
    [ -f "$arquivo" ] || {
        echo "vocabulario incompleto: $arquivo nao existe" >&2
        exit 2
    }
done

echo "== montando $DESTINO =="
rm -rf "$DESTINO"
mkdir -p "$DESTINO"

cp -r "$HARNESS_ROOT/glossary" "$DESTINO/"
cp -r "$HARNESS_ROOT/classification" "$DESTINO/"

{
    # `--match 'vocab-v*'` e obrigatorio: sem ele o `describe` acha a tag do
    # binario e o pacote do vocabulario se anuncia como `v0.7.0`, que e a
    # versao de outra coisa. Dois releases no mesmo repositorio compartilham o
    # espaco de tags, e o filtro e o que os mantem distinguiveis.
    echo "vocabulario  $(git -C "$HARNESS_ROOT" describe --tags --match 'vocab-v*' --always --dirty 2>/dev/null || echo sem-tag)"
    echo "commit       $(git -C "$HARNESS_ROOT" rev-parse HEAD 2>/dev/null || echo '-')"
    echo "glossario    $(versao_de "$GLOSSARIO")  $(sha256_de "$GLOSSARIO")"
    echo "catalogo     $(versao_de "$CATALOGO")  $(sha256_de "$CATALOGO")"
} > "$DESTINO/VERSION"

echo
cat "$DESTINO/VERSION"
echo
echo "pronto."
