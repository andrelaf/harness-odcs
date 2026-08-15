# shellcheck shell=sh
# Fonte unica de configuracao de ambiente.
#
# Este arquivo e *sourced*, nunca executado, e nao contem logica de fluxo.
# Todo valor que o harness precisa saber sobre o mundo externo nasce aqui —
# inclusive a versao da imagem, que o binario le por variavel de ambiente em
# vez de carregar um default embutido. Um default no codigo seria uma segunda
# fonte de verdade.

: "${HARNESS_ROOT:?env.sh precisa ser carregado com HARNESS_ROOT definido}"

# --- As tres raizes ----------------------------------------------------------
#
# Eram uma so, e uma so nao sobrevive a sair deste repositorio. Um repositorio
# de contratos tem o dado, mas nao tem o binario nem o vocabulario — e nao
# deveria ter: quem escreve o contrato nao pode escrever o criterio que o julga.
#
#   HARNESS_ROOT   os dados     — `contracts/`, e onde `trace/` e `evidence/`
#                                 sao escritos. Aqui, o repositorio; la, o
#                                 checkout do repositorio de contratos.
#   HARNESS_HOME   a ferramenta — o binario e estes scripts. Aqui, o mesmo
#                                 diretorio; la, o pacote extraido do release.
#   HARNESS_VOCAB  o criterio   — `glossary/` e `classification/`.
#
# Os defaults fazem as tres coincidirem, que e o caso deste repositorio: nada
# muda para quem opera daqui. Fora daqui, o entrypoint do pacote as separa.
: "${HARNESS_HOME:=$HARNESS_ROOT}"
: "${HARNESS_VOCAB:=$HARNESS_HOME}"

# --- Motor de validacao ODCS -------------------------------------------------
# Fixado por versao E digest. `latest` destruiria a reprodutibilidade: o mesmo
# contrato poderia passar hoje e falhar amanha sem nenhum commit. Trocar de
# versao e decisao registrada, nao efeito de `docker pull`.
DC_IMAGE="datacontract/cli:1.1.0"
DC_DIGEST="sha256:f7fa02d649f4992dd8297bb428ece7403d688e881cf4a386673e250cb678657b"

# Porta do editor pessoal (`datacontract edit`). Default do CLI e 4243.
DC_EDIT_PORT="4243"

# --- Editor compartilhado da organizacao -------------------------------------
# A porta interna da imagem e 4173, nao 80: o nginx so escuta na 4173, e
# mapear 80 sobe o container sem resposta nenhuma.
DC_EDITOR_IMAGE="datacontract/editor:latest"
DC_EDITOR_PORT_INTERNA="4173"
DC_EDITOR_PORT="8080"

# --- Plataforma --------------------------------------------------------------
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*)
        EXE=".exe"
        # Caminho nativo para quem nao entende POSIX: cargo, docker e o proprio
        # binario do harness sao programas Windows.
        #
        # MSYS_NO_PATHCONV NAO e exportado aqui de proposito. Exportado, ele
        # desativa a conversao de caminho para TODO binario Windows, e o cargo
        # passa a receber `/c/repos/...`, que nao existe para ele. A variavel e
        # aplicada inline, so nas chamadas de `docker` que fazem mount.
        HARNESS_ROOT_NATIVE="$(cygpath -w "$HARNESS_ROOT" | tr '\\' '/')"
        HARNESS_VOCAB_NATIVE="$(cygpath -w "$HARNESS_VOCAB" | tr '\\' '/')"
        ;;
    *)
        EXE=""
        HARNESS_ROOT_NATIVE="$HARNESS_ROOT"
        HARNESS_VOCAB_NATIVE="$HARNESS_VOCAB"
        ;;
esac

# Compilado em `target/` quando se opera do proprio repositorio; em `bin/`
# quando se opera do pacote. Quem existir vence — e por isso que o mesmo
# `env.sh` serve aos dois entrypoints.
if [ -x "$HARNESS_HOME/bin/harness-odcs$EXE" ]; then
    HARNESS_BIN="$HARNESS_HOME/bin/harness-odcs$EXE"
else
    HARNESS_BIN="$HARNESS_HOME/target/debug/harness-odcs$EXE"
fi

export DC_IMAGE DC_DIGEST DC_EDIT_PORT
export DC_EDITOR_IMAGE DC_EDITOR_PORT DC_EDITOR_PORT_INTERNA
export EXE HARNESS_BIN HARNESS_ROOT HARNESS_ROOT_NATIVE
export HARNESS_HOME HARNESS_VOCAB HARNESS_VOCAB_NATIVE
