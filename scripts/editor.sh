#!/usr/bin/env sh
# Sobe um editor ODCS.
#
#   ./scripts/editor.sh pessoal [contrato]   editor ligado ao arquivo do repo
#   ./scripts/editor.sh org                  editor compartilhado, sem repo
#   ./scripts/editor.sh parar                derruba os dois
#
# Modo `pessoal`: o "salvar" do editor escreve direto no .yaml do repositorio.
# Modo `org`: aplicacao estatica, roda no browser, a pessoa baixa o YAML. E o
# modo recomendado para uso corporativo — o `pessoal` monta o repo de UMA
# pessoa e edita UM arquivo, entao nao tem como servir a organizacao.
#
# Nenhum dos dois tem autenticacao. Publicar em rede exige VPN ou proxy com SSO.
set -eu

HARNESS_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export HARNESS_ROOT
. "$HARNESS_ROOT/scripts/env.sh"

modo="${1:-pessoal}"
contrato="${2:-contracts/clientes.odcs.yaml}"

case "$modo" in
    pessoal)
        docker rm -f odcs-editor >/dev/null 2>&1 || true
        echo "editor pessoal em http://localhost:$DC_EDIT_PORT  (arquivo: $contrato)"
        exec env MSYS_NO_PATHCONV=1 docker run --rm --name odcs-editor \
            -p "$DC_EDIT_PORT:$DC_EDIT_PORT" \
            -v "$HARNESS_ROOT_NATIVE:/home/datacontract" \
            "$DC_IMAGE" edit "$contrato" --host 0.0.0.0 --no-open
        ;;

    org)
        docker rm -f odcs-editor-org >/dev/null 2>&1 || true
        # A porta interna e 4173, nao 80: mapear 80 sobe o container mudo.
        docker run -d --name odcs-editor-org \
            -p "$DC_EDITOR_PORT:$DC_EDITOR_PORT_INTERNA" \
            "$DC_EDITOR_IMAGE" >/dev/null
        echo "editor compartilhado em http://localhost:$DC_EDITOR_PORT"
        echo "AI desligada por padrao — nao defina AI_API_KEY: mandaria o"
        echo "contrato para um LLM externo e persistiria segredo em env."
        ;;

    parar)
        docker rm -f odcs-editor odcs-editor-org >/dev/null 2>&1 || true
        echo "editores parados"
        ;;

    *)
        echo "uso: $0 [pessoal|org|parar] [contrato]" >&2
        exit 2
        ;;
esac
