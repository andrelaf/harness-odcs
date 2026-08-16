#!/usr/bin/env sh
# Materializa um repositorio de contratos a partir do template deste projeto.
#
#   ./scripts/novo-repo-de-contratos.sh /caminho/do/repo [versao]
#
# O repositorio de contratos nasce **em branco**, e tudo que ele precisa para
# existir mora aqui: workflow, CODEOWNERS, template de pull request, scripts,
# `.gitignore`, `.gitattributes` e o `harness.lock`. Nada e escrito a mao la.
#
# Por que aqui e nao la: o que esses arquivos fazem depende do contrato de saida
# do harness — exit codes, formatos de relatorio, nomes de comando. Se eles
# morassem no repositorio de contratos, uma versao nova do harness poderia
# quebra-los sem que ninguem percebesse ate o proximo pull request. Versionados
# junto do binario que os alimenta, os dois mudam no mesmo commit.
#
# E o que torna a porta para o Azure DevOps barata: `azure-pipelines.yml` esta
# no mesmo template, ao lado do workflow do GitHub, e os dois chamam exatamente
# os mesmos comandos.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TEMPLATE="$HARNESS_ROOT/templates/repo-de-contratos"

DESTINO="${1:-}"
[ -n "$DESTINO" ] || {
    echo "uso: $0 /caminho/do/repo-de-contratos [versao]" >&2
    echo "     a versao e opcional; sem ela, usa o ultimo release publicado" >&2
    exit 2
}

REPO="${HARNESS_REPO_ORIGEM:-andrelaf/harness-odcs}"
VERSAO="${2:-}"

# --- A versao fixada ---------------------------------------------------------
#
# Descoberta aqui, gravada la. O repositorio de contratos nunca "pega a ultima":
# ele aponta para uma versao e um sha256 especificos, e subir de versao passa a
# ser um pull request revisado.
echo "== resolvendo a versao do pacote =="
if [ -z "$VERSAO" ]; then
    VERSAO="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' | head -1)"
    [ -n "$VERSAO" ] || { echo "nao consegui descobrir o ultimo release de $REPO" >&2; exit 1; }
fi

SHA="$(curl -fsSL "https://github.com/$REPO/releases/download/$VERSAO/harness-odcs-linux-x64.tar.gz.sha256" \
    | cut -d' ' -f1)"
[ -n "$SHA" ] || { echo "release $VERSAO nao publicou o sha256 — foi um release completo?" >&2; exit 1; }
echo "  $REPO@$VERSAO"
echo "  sha256 $SHA"

# --- O template --------------------------------------------------------------
echo
echo "== copiando o template para $DESTINO =="
mkdir -p "$DESTINO"
# `-R` de diretorio inteiro, incluindo os que comecam com ponto. `cp -R src/.`
# copia o conteudo, e nao o diretorio, que e o que se quer aqui.
cp -R "$TEMPLATE/." "$DESTINO/"

cat > "$DESTINO/harness.lock" <<EOF
# Pacote de validacao — versao fixada.
#
# Gerado por \`novo-repo-de-contratos.sh\`. Este arquivo e a unica declaracao de
# qual criterio julga os contratos deste repositorio: o pipeline le daqui, e
# quem roda a verificacao na propria maquina le daqui tambem.
#
# **Versao E sha256.** So a versao nao basta — uma tag pode ser movida, e no dia
# em que for, o mesmo contrato passa a ser julgado por outro criterio sem que
# nada aqui tenha mudado.
#
# Subir de versao pode reclassificar campos sem que nenhum contrato mude. Por
# isso e um pull request revisado, com dono proprio no CODEOWNERS.

HARNESS_REPO=$REPO
HARNESS_VERSAO=$VERSAO
HARNESS_SHA256=$SHA
EOF

mkdir -p "$DESTINO/contracts"

# Bit de execucao: `core.fileMode` e `false` no Windows, entao `chmod` local nao
# vira commit. Sem isto o runner recebe 100644 e o job falha com "permission
# denied" — que foi como o primeiro release deste projeto reprovou.
chmod +x "$DESTINO/scripts/"*.sh 2>/dev/null || true

echo
echo "== pronto =="
echo
echo "  cd $DESTINO"
echo "  git init && git add -A"
echo "  git update-index --chmod=+x scripts/*.sh"
echo "  git commit -m 'chore: estrutura do repositorio de contratos'"
echo
echo "Depois, no GitHub:"
echo "  1. troque os times em .github/CODEOWNERS pelos da sua organizacao"
echo "  2. Settings > Rules > Rulesets: exija pull request e o status check \`check\`"
echo "  3. abra o primeiro contrato em contracts/<dominio>/<contrato>/contract.odcs.yaml"
echo
echo "O passo a passo completo esta em docs/bootstrap-repo-contratos.md"
