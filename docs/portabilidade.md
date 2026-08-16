# Portabilidade — a prova

O brief chama a portabilidade de "o grande item da semana", e define a regra de
ouro: **a política do workflow não pode morar em config de IDE.** Este documento
registra como isso foi garantido por construção, e a evidência que existe.

---

## A regra, e como ela é sustentada

A política vive em **dois lugares, ambos versionados e nenhum deles específico de
ambiente**:

| Onde | O quê |
|---|---|
| `run.sh` | despacha argumentos; **nenhum `if` de negócio** |
| binário Rust | ordem das fases, teto de passos, transições, gate, classificação |

O que **não** existe neste repositório, e a ausência é a prova:

```
.cursorrules        .vscode/tasks.json      .idea/
.devcontainer/      Makefile com regra de fluxo
```

`run.sh` inteiro tem 18 linhas e faz três coisas: resolve a raiz, carrega o
ambiente, garante o binário e repassa `argv`. O comentário no topo diz o motivo:

> *"Se este script ganhar um `if` de negócio, a política passou a existir em dois
> lugares e a portabilidade entre IDEs quebra."*

A configuração de ambiente também tem fonte única: `scripts/env.sh`, e o binário
**falha** se as variáveis faltarem, em vez de carregar um default embutido. Um
default no código seria uma segunda verdade, e a versão da imagem passaria a
divergir do que os scripts declaram.

---

## Os ambientes exercitados

| Ambiente | Como | Evidência |
|---|---|---|
| **Claude Code** (agente) | `./run.sh next`, ciclo completo das 4 features | `trace/` — 14 runs |
| **VS Code / git bash** (manual) | mesmo ponto de entrada, execução humana | mesmos comandos, mesmos exit codes |
| **GitHub Actions** (CI) | `./run.sh check` num runner Ubuntu limpo | `.github/workflows/contrato-pr.yml` |
| **Repositório separado** | pacote extraído, `harness.sh check` contra outro repo | `andrelaf/data-contracts` |

O terceiro e o quarto não estavam no brief, e são a evidência mais forte: **o
mesmo julgamento atravessou um sistema operacional diferente, um repositório
diferente e um binário distribuído — sem uma linha de política reescrita.**

### O teste que fecha o argumento

O mesmo contrato, verificado em três lugares, devolve o **mesmo veredito e o mesmo
exit code**:

```
Windows, git bash, binário local     → BLOQUEADO (5), 2 lacunas
Ubuntu runner, pacote v0.4.0         → BLOQUEADO (5), 2 lacunas
Outro repositório, sem Rust nem vocabulário → BLOQUEADO (5), 2 lacunas
```

E o laudo emitido nos três é **byte a byte idêntico** — é essa propriedade que
permite ao `check` exigir que o repositório contenha o que ele produziria.

---

## O que a portabilidade custou

Ela não foi de graça, e o preço apareceu no dia em que o harness saiu daqui.

**`HARNESS_ROOT` fazia dois trabalhos.** Era a raiz dos dados (`contracts/`,
`trace/`, `evidence/`) e a do código (`target/`, `scripts/`) ao mesmo tempo. Num
repositório de contratos não existe `target/`. Viraram três variáveis —
`HARNESS_ROOT`, `HARNESS_HOME`, `HARNESS_VOCAB` —, com defaults que fazem as três
coincidirem aqui dentro. Detalhes em [`distribuicao.md`](distribuicao.md).

**O bit de execução nunca esteve no git.** `core.fileMode` é `false` no Windows,
então `chmod +x` local nunca virou commit — e **nenhum** script deste
repositório tinha o bit, inclusive `run.sh`, desde a Semana 1. Só apareceu no
primeiro checkout limpo em Linux, com *permission denied*. Corrigido com
`git update-index --chmod=+x` e anotado no `.gitattributes`.

**Permissão de arquivo dentro do container.** A imagem do `datacontract-cli` roda
como `nonroot`, e escreve o relatório no volume montado. No Docker Desktop
(Windows, macOS) dono e modo são ignorados; num runner Linux, não — o diretório
nascia `755` do usuário do CI e o CLI reprovava sem deixar arquivo. Passou em
quatro semanas de execução local e só o primeiro pull request encontrou.

**Fim de linha.** Sem `.gitattributes`, um `.sh` com CRLF não executa: o `\r`
entra no shebang. Fixado `eol=lf` para `.sh`, `.yaml` e `.yml` — este último
importa porque o sha256 do contrato entra no nome do laudo, e fim de linha
diferente é conteúdo diferente.

**A lição.** Portabilidade estrutural — não deixar política em config de IDE — é
condição necessária e barata. O que custou foram as **suposições de ambiente**:
caminho, permissão, dono de arquivo, fim de linha. Nenhuma delas aparece enquanto
se roda numa máquina só, e todas aparecem juntas no primeiro CI.
