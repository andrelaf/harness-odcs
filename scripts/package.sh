#!/usr/bin/env sh
# Monta o pacote distribuivel do harness.
#
# O mesmo script roda na maquina de quem desenvolve e no job de release. Se
# fossem dois, o pacote testado localmente nao seria o pacote publicado — e a
# diferenca so apareceria no repositorio de contratos de outra pessoa.
#
#   ./scripts/package.sh [destino]
#
# Produz:
#
#   <destino>/
#     harness.sh                     entrypoint — nao compila nada
#     bin/harness-odcs[.exe]         o binario, em release
#     scripts/env.sh                 a configuracao, fonte unica
#     glossary/                      o criterio
#     classification/                o criterio
#     VERSION                        versao, commit e sha256 do vocabulario
#
# O vocabulario vai **dentro** do pacote nesta versao. E uma escolha reversivel,
# nao uma conviccao: `HARNESS_VOCAB` aponta para onde se quiser, e no dia em que
# o glossario tiver repositorio proprio, o pacote deixa de carrega-lo e o
# pipeline passa a fixar duas versoes em vez de uma. O que nao pode e o
# vocabulario morar no repositorio de contratos: quem escreve o contrato nao
# escreve o criterio que o julga.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export HARNESS_ROOT
. "$HARNESS_ROOT/scripts/env.sh"

DESTINO="${1:-$HARNESS_ROOT/dist/harness-odcs}"

# `sha256sum` no Linux e no git bash; `shasum -a 256` no macOS. Sem isto o
# VERSION sairia vazio justamente na maquina de quem for testar num Mac.
sha256_de() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

echo "== compilando =="
cd "$HARNESS_ROOT"
cargo build --release --quiet

echo "== montando $DESTINO =="
rm -rf "$DESTINO"
mkdir -p "$DESTINO/bin" "$DESTINO/scripts"

cp "$HARNESS_ROOT/target/release/harness-odcs$EXE" "$DESTINO/bin/"
cp "$HARNESS_ROOT/scripts/env.sh" "$DESTINO/scripts/"
cp "$HARNESS_ROOT/scripts/imagem.sh" "$DESTINO/scripts/"
cp "$HARNESS_ROOT/scripts/harness.sh" "$DESTINO/harness.sh"
chmod +x "$DESTINO/harness.sh" "$DESTINO/scripts/imagem.sh" "$DESTINO/bin/harness-odcs$EXE"

cp -r "$HARNESS_ROOT/glossary" "$DESTINO/"
cp -r "$HARNESS_ROOT/classification" "$DESTINO/"

# A procedencia, dentro do pacote. Sem isto, um pacote extraido num runner e
# um diretorio anonimo, e "qual criterio julgou este contrato?" deixa de ter
# resposta fora do laudo.
{
    echo "harness-odcs $(git -C "$HARNESS_ROOT" describe --tags --always --dirty 2>/dev/null || echo sem-tag)"
    echo "commit       $(git -C "$HARNESS_ROOT" rev-parse HEAD 2>/dev/null || echo '-')"
    echo "glossario    $(sha256_de "$HARNESS_ROOT/glossary/glossario.yaml")"
    echo "catalogo     $(sha256_de "$HARNESS_ROOT/classification/catalogo-lgpd.yaml")"
    echo "dc_image     $DC_IMAGE"
    echo "dc_digest    $DC_DIGEST"
} > "$DESTINO/VERSION"

echo
cat "$DESTINO/VERSION"
echo
echo "pacote pronto: $DESTINO"
