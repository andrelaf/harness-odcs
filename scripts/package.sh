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
#     VERSION                        versao, commit e a imagem do motor
#
# --- O vocabulario nao vem mais aqui
#
# Ele vinha, e sair foi o plano desde entao: `HARNESS_VOCAB` sempre apontou para
# onde se quisesse, e este cabecalho ja dizia que "no dia em que o glossario
# tiver versao propria, o pacote deixa de carrega-lo e o pipeline passa a fixar
# duas versoes em vez de uma". E este dia.
#
# O motivo e cadencia, nao arrumacao: com o vocabulario aqui dentro, cadastrar
# `segmento` exigia compilar Rust e publicar um release do binario. Um data
# steward nao deveria depender disso para acrescentar um termo.
#
# Agora sao dois pacotes, dois pins e dois donos possiveis — `package-vocabulario.sh`
# monta o outro. O que continua valendo e que o vocabulario **nao** mora no
# repositorio de contratos: quem escreve o contrato nao escreve o criterio que
# o julga.
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

# A procedencia, dentro do pacote. Sem isto, um pacote extraido num runner e
# um diretorio anonimo.
#
# O sha256 do glossario e do catalogo **saiu daqui** junto com os arquivos: a
# procedencia do criterio agora e do pacote do vocabulario, que tem VERSION
# propria. Manter os hashes aqui seria afirmar sobre um conteudo que este
# pacote nao carrega e nao controla — e que muda numa cadencia diferente.
{
    echo "harness-odcs $(git -C "$HARNESS_ROOT" describe --tags --always --dirty 2>/dev/null || echo sem-tag)"
    echo "commit       $(git -C "$HARNESS_ROOT" rev-parse HEAD 2>/dev/null || echo '-')"
    echo "dc_image     $DC_IMAGE"
    echo "dc_digest    $DC_DIGEST"
} > "$DESTINO/VERSION"

echo
cat "$DESTINO/VERSION"
echo
echo "pacote pronto: $DESTINO"
