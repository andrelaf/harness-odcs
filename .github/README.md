# Verificação de contrato em pull request

Como o harness roda no CI, o que ele decide, o que ele deliberadamente **não**
decide, e o que você precisa configurar no repositório para que a decisão
humana seja mecânica em vez de social.

Arquivos desta pasta:

| Arquivo | Papel |
|---|---|
| [`workflows/contrato-pr.yml`](workflows/contrato-pr.yml) | O job. Não contém regra de negócio — chama o binário e desenha o resultado. |
| [`workflows/pr-convencao.yml`](workflows/pr-convencao.yml) | Confere o nome da branch. A regra está em [`docs/git-flow.md`](../docs/git-flow.md). |
| [`CODEOWNERS`](CODEOWNERS) | Quem revisa o quê. É o que transforma "alguém precisa aprovar" em "esta pessoa precisa aprovar". |
| [`pull_request_template.md`](pull_request_template.md) | O que quem abre o PR precisa dizer, e o que quem aprova precisa ter lido. |

Os dois workflows disparam em **pull request para `main`** — não há branch de
longa duração além dela, e nada acontece por push direto. O ciclo inteiro está
em [`docs/git-flow.md`](../docs/git-flow.md).

---

## O desenho em uma frase

**A política mora no binário; o CI só desenha.** O workflow chama
`./run.sh check`, que produz um `report.json` neutro de plataforma, e depois
`./run.sh report` para redesenhar esse mesmo arquivo nos formatos que o GitHub
entende. Nenhum julgamento acontece em YAML.

Três consequências, e são elas que justificam o desenho:

1. **O CI e a máquina de quem desenvolve concordam.** `check` chama as mesmas
   funções que as fases chamam (`defeitos_do_caminho`, `ler_veredito`,
   `compor`, `lint_do_enriquecido`). Não há uma segunda implementação da regra
   esperando divergir na primeira mudança.
2. **Uma verificação, três desenhos.** 99,6% do custo medido do harness é
   partida de container. Se cada formato reexecutasse o `check`, comentar,
   anotar e resumir custariam três verificações. Por isso `check --saida` grava
   e `report` só relê.
3. **Portar para outro CI custa um renderizador.** O que muda entre GitHub e
   Azure DevOps é uma função de ~30 linhas em `src/check.rs` e este arquivo
   YAML. O veredito não se move. Veja [Azure DevOps](#portar-para-azure-devops).

---

## O que o job faz — e o que ele não faz

```
pull request  →  check  →  report.json  →  ┌ anotações no diff       (--formato github)
                                           ├ comentário no PR        (--formato markdown)
                                           ├ resumo do job           (--formato markdown)
                                           └ artefatos: evidence/ trace/
```

**Não faz, de propósito:**

- **Não escreve no contrato.** `check` não toca `contracts/`, não toca `state/`,
  não commita. Ele calcula o contrato enriquecido e o laudo e os deixa em
  `evidence/`, como **proposta**. Quem aceita é quem abriu o PR.
- **Não avança o fluxo.** Sem isso, três PRs abertos ao mesmo tempo disputariam
  `state/progress.json` e cada run de CI viraria um commit conflitando com os
  outros dois.
- **Não aprova gate.** `state/aprovacoes.json` é ignorado pelo `check` — está no
  repositório, e quem abriu o PR pode commitá-lo. Num pull request isso seria
  auto-aprovação. Quem tem autoridade sobre o gate é a revisão de CODEOWNER.
- **Não tem permissão para nada disso.** O job roda com `contents: read`. Ainda
  que o código quisesse commitar, o token não deixa.

---

## O veredito, e o que o GitHub faz com ele

O exit code do `check` é o contrato com o CI. Ele também está dentro do
`report.json`, no campo `exit_code`, para quem preferir ler o arquivo.

| Exit | Veredito | O que aconteceu | O job |
|---|---|---|---|
| `0` | `pass` | Contrato válido, nomeado certo, classificado sem pendência. | verde |
| `1` | `fail` | Reprovou. Motivo em `defeitos[]`, ancorado no arquivo do diff. | vermelho |
| `5` | `bloqueado` | Nada errado — falta decisão humana. Motivo em `gate[]`. | **verde, com `::notice`** |

**`5` não reprova o job**, e essa é a escolha menos óbvia da tabela. Uma lacuna
não é um erro de quem abriu o PR: é um campo que o glossário não cobre e sobre o
qual alguém precisa decidir. Marcar o PR de vermelho diria à pessoa errada que
ela errou.

Quem segura o merge nesse caso é a branch protection exigindo revisão de
CODEOWNER — a autoridade certa, e a que fica registrada no histórico. Se você
prefere que o próprio check fique vermelho até a aprovação, veja
[a variante](#variante-o-check-vermelho-até-a-aprovação) abaixo, com o custo
dela dito em voz alta.

---

## Configuração obrigatória no repositório

O workflow sozinho **não bloqueia nada**. Sem os quatro itens abaixo, ele é um
comentário bonito num PR que qualquer pessoa faz merge assim mesmo.

### 1. CODEOWNERS com times que existem

Substitua `@sua-org/...` em [`CODEOWNERS`](CODEOWNERS) pelos times reais.
**Time inexistente não bloqueia nada** — é a falha mais silenciosa desse
arquivo. Confira em *Settings → Branches* se o GitHub reconhece os donos (o
editor de CODEOWNERS marca linhas inválidas).

Os donos de `/glossary/` e `/classification/` precisam ser **diferentes** de
quem escreve o contrato. Se o autor puder alterar o catálogo no mesmo PR, ele
reclassifica o próprio campo e o gate do F4 deixa de significar qualquer coisa.

### 2. Branch protection na branch de destino

*Settings → Branches → Add rule*, na branch para onde os contratos vão:

| Opção | Por quê |
|---|---|
| **Require a pull request before merging** | Sem isso, um push direto pula a verificação inteira. |
| **Require review from Code Owners** | É o que dá autoridade à revisão. É este item — não o CI — que segura o merge no exit `5`. |
| **Dismiss stale pull request approvals when new commits are pushed** | **Crítico.** O pedido de gate é identificado pelo hash do seu conteúdo: se o contrato, o glossário ou o catálogo mudarem, o pedido é outro. Sem isso, uma aprovação dada numa lacuna carregaria em silêncio para um contrato diferente. |
| **Require status checks to pass** → marque `check` e `branch` | Os nomes são os dos jobs: `check` (workflow `contrato`) e `branch` (workflow `convencao`). Eles só aparecem na lista depois de terem rodado ao menos uma vez. |
| **Require branches to be up to date before merging** | O veredito é calculado sobre um sha256 específico do contrato. Numa branch desatualizada, ele responde por um conteúdo que não é o que vai entrar. |

### 3. Layout de diretório que o CODEOWNERS consegue rotear

`contracts/<domínio>/<contrato>/contract.odcs.yaml`. Sem o nível de domínio, ou
uma pessoa aprova todo contrato da empresa, ou o CODEOWNERS cresce uma linha por
contrato até ninguém mais mantê-lo. O `check` emite um **aviso** (não reprova)
quando o contrato está raso.

### 4. Nada além do `GITHUB_TOKEN`

Não há secret a configurar. O `gh pr comment` usa `github.token`, e as
permissões declaradas no workflow são o mínimo: `contents: read`,
`pull-requests: write`.

---

## Rodar localmente o que o CI roda

É o mesmo binário, sem nenhuma diferença de comportamento:

```bash
./run.sh check                      # veredito legível no terminal
./run.sh check --formato markdown   # exatamente o comentário que o PR receberia
./run.sh check --json               # o report.json no stdout
echo $?                             # 0 · 1 · 5
```

Com mais de um contrato no repositório, escolha o alvo — sem a flag, o `check`
só resolve sozinho quando não há ambiguidade:

```bash
./run.sh check --contrato contracts/clientes/contract.odcs.yaml
```

Para redesenhar um relatório já produzido (é o que os passos seguintes do job
fazem, sem reexecutar a verificação):

```bash
./run.sh check --saida /tmp/report.json --formato github
./run.sh report /tmp/report.json --formato markdown
```

`report` **sai sempre com `0`**: um renderizador que reprova confundiria o
veredito do contrato com o sucesso do desenho. O veredito vive no `check` — e
dentro do próprio JSON.

---

## Variante: o check vermelho até a aprovação

O padrão deixa o merge com a branch protection. Se a sua organização exige que o
status check em si fique vermelho enquanto houver pendência, troque o passo
`veredito` por:

```yaml
      - name: veredito
        if: always()
        env:
          GH_TOKEN: ${{ github.token }}
          PR: ${{ github.event.pull_request.number }}
        run: |
          codigo="${{ steps.check.outputs.codigo }}"
          case "$codigo" in
            0) exit 0 ;;
            5)
              decisao="$(gh pr view "$PR" --json reviewDecision -q .reviewDecision)"
              if [ "$decisao" = "APPROVED" ]; then
                echo "::notice::gate liberado pela revisao de CODEOWNER"
                exit 0
              fi
              echo "::error::gate aberto, aguardando revisao de CODEOWNER"
              exit 1
              ;;
            *) echo "::error::verificacao reprovou (exit $codigo)"; exit 1 ;;
          esac
```

E acrescente o gatilho de revisão, senão o check só reavalia no próximo push:

```yaml
on:
  pull_request:
    paths: [ ... ]
  pull_request_review:
    types: [submitted, dismissed]
```

**O custo, dito antes de você escolher:** `paths:` não se aplica ao evento
`pull_request_review`, então o job passa a rodar em toda revisão de todo PR — e
cada run é uma verificação inteira, com partida de container. E, mais de fundo,
isso faz o CI reimplementar em `bash` uma decisão que o GitHub já toma na branch
protection, com a diferença de que a versão em `bash` não aparece em lugar
nenhum na tela de configuração do repositório. Prefira o padrão a menos que
alguém exija o contrário por escrito.

---

## Portar para Azure DevOps

> **Este é o destino previsto, e o GitHub aqui é a prova de conceito.** Numa
> organização, o CI/CD mora no Azure DevOps e os aprovadores são **grupos do
> Entra ID (AD)**, não times de repositório. Nada disso muda o harness — está
> escrito abaixo para que a portabilidade seja uma decisão já tomada, e não uma
> descoberta no dia da migração.

Muda **um arquivo YAML e uma função**. O julgamento não se move, porque ele está
no binário e o `report.json` é neutro de plataforma.

| GitHub | Azure DevOps |
|---|---|
| `.github/workflows/contrato-pr.yml` | `azure-pipelines.yml`, com os mesmos dois passos (`check --saida`, depois `report`) |
| `--formato github` → `::error file=…` | um `Formato::Azure` → `##vso[task.logissue type=error;sourcepath=…]` |
| `gh pr comment --edit-last` | `az repos pr` ou a REST de *pull request threads*, com o mesmo `--formato markdown` |
| `CODEOWNERS` + *Require review from Code Owners* | *Branch policies → Automatically included reviewers*, por caminho — ver abaixo |
| `GITHUB_STEP_SUMMARY` | `##vso[task.uploadsummary]` |
| Artefatos do `actions/upload-artifact` | `PublishPipelineArtifact@1` sobre `evidence/` e `trace/` |

O renderizador novo é uma variante em `desenhar()` (`src/main.rs`) e uma função
irmã de `github()` (`src/check.rs`). Nada mais precisa saber que existe outro
CI — e é para isso que o `report.json` está no meio do caminho.

### Os aprovadores, por grupo do AD

É aqui que a migração deixa de ser cosmética. No GitHub, o dono de um caminho é
um time do repositório escrito em `CODEOWNERS`. No Azure DevOps, o equivalente
não é um arquivo versionado: é uma **política de branch por caminho**, e o
revisor exigido é um **grupo de segurança do Entra ID** já existente na
organização — o mesmo que a área de privacidade usa para outras coisas.

O mapeamento que preserva o desenho do gate:

| Caminho | Quem aprova | Política |
|---|---|---|
| `/contracts/<domínio>/` | grupo do AD do dono do dado daquele domínio | *Automatically included reviewers*, `Required` |
| `/glossary/` | grupo do AD dos *data stewards* | *Required*, e **distinto** do grupo acima |
| `/classification/` | grupo do AD de privacidade / encarregado de dados | *Required*, e **distinto** do grupo acima |

Três pontos que precisam sobreviver à tradução, porque são deles que o gate
tira o significado:

1. **Separação de grupos, não de pessoas.** O grupo que aprova o contrato não
   pode ser o mesmo que aprova o glossário e o catálogo. Se for, o autor fecha a
   própria lacuna e reclassifica o próprio campo — e o F4 vira carimbo. Isso é
   mais fácil de errar no Azure DevOps do que no GitHub: aninhamento de grupos
   do AD é invisível na tela de políticas, e um grupo guarda-chuva de
   "engenharia de dados" que contenha os três anula a separação sem que nenhuma
   configuração pareça errada. Confira a associação efetiva, não o nome.
2. **`Reset approvals on new pushes`** ligado. É o equivalente ao *dismiss stale
   approvals*, e vale pelo mesmo motivo: o pedido de gate é identificado pelo
   hash do seu conteúdo, então aprovação presa a um conteúdo antigo é aprovação
   de outra coisa.
3. **A aprovação continua sendo do pull request, não do pipeline.**
   *Environments* e *Approvals and checks* do Azure DevOps aprovam **deploy** —
   uma etapa depois. Usá-los para o gate de classificação colocaria a decisão
   fora do PR, longe do diff e do laudo que o revisor precisa ler, e o registro
   ficaria no histórico do pipeline em vez de no do repositório. O gate do F4 é
   revisão de código, e é onde ele deve ficar.

Custo estimado da porta: o renderizador (~30 linhas e seus testes), o
`azure-pipelines.yml` (equivalente 1:1 ao workflow) e a configuração das
políticas. O `check`, o `report.json`, os exit codes e todas as regras ficam
como estão — o que é, no fim, o teste de que a separação entre política e
desenho valeu a pena.

---

## Falhas comuns

**O comentário não aparece em PR vindo de fork.** O `GITHUB_TOKEN` é read-only
nesse caso e o passo de comentário falha. O resumo do job e os artefatos
continuam funcionando. **Não** troque para `pull_request_target` para contornar:
isso executaria código do fork com um token de escrita.

**O check não aparece na lista de status checks obrigatórios.** O GitHub só
oferece checks que já rodaram pelo menos uma vez naquela branch. Abra um PR de
teste primeiro, depois configure a branch protection.

**O digest da imagem diverge e o `bootstrap.sh` falha.** É proposital: o
`datacontract-cli` está fixado por digest em `scripts/env.sh`, e não por tag. Um
contrato não pode passar hoje e reprovar amanhã sem nenhum commit. Se a
divergência for uma atualização que você quer, atualize `DC_DIGEST` — num commit
que diga isso.

**O gate reabre depois de uma aprovação.** É o comportamento correto: o hash do
pedido cobre o texto dos itens. Reescrever a frase de um `detalhe` invalida
aprovações antigas. Falha fechada, que é o lado certo de errar.

**As actions não estão fixadas por SHA.** `actions/checkout@v4` e
`Swatinem/rust-cache@v2` usam tag móvel. Para um repositório que guarda
contratos de dados, vale fixar por SHA — é a mesma disciplina que já se aplica à
imagem do `datacontract-cli`, e a inconsistência aqui é deliberada apenas
enquanto isto é um projeto de demonstração.
