# Criar um repositório de contratos, do zero

Passo a passo para levantar o repositório onde os contratos de dados vão morar.
Ele **nasce em branco**: tudo que ele precisa para existir — workflow,
`CODEOWNERS`, template de pull request, scripts, `harness.lock` — mora aqui, em
[`templates/repo-de-contratos/`](../templates/repo-de-contratos/), e é
materializado por um script.

## Por que os arquivos moram aqui, e não lá

O que esses arquivos fazem depende do **contrato de saída do harness**: exit
codes, nomes de comando, formato do `report.json`, chaves do `harness.lock`. Se
morassem no repositório de contratos, uma versão nova do harness poderia
quebrá-los sem que ninguém percebesse até o próximo pull request.

Versionados junto do binário que os alimenta, os dois mudam no mesmo commit — e
o `azure-pipelines.yml` fica ao lado do workflow do GitHub, o que torna a porta
entre as plataformas um arquivo, não um projeto.

---

## 1. Materializar

```bash
./scripts/novo-repo-de-contratos.sh /caminho/do/data-contracts
```

Sem argumento de versão, o script descobre o **último release publicado** e grava
`harness.lock` com a tag e o sha256 daquele pacote. Para fixar outra:

```bash
./scripts/novo-repo-de-contratos.sh /caminho/do/data-contracts v0.4.0
```

O que ele deixa lá:

```
.github/workflows/contrato-pr.yml   a esteira (GitHub Actions)
.github/CODEOWNERS                  quem revisa o quê
.github/pull_request_template.md    o que quem abre e quem aprova precisam dizer
azure-pipelines.yml                 a mesma esteira, para Azure DevOps
scripts/preparar.sh                 baixa o pacote fixado (opcional, para dev)
scripts/verificar.sh                antecipa o veredito (opcional, para dev)
harness.lock                        a versão do critério — gerada, não copiada
contracts/                          vazio, à espera do primeiro contrato
.gitignore  .gitattributes  README.md
```

## 2. Primeiro commit

```bash
cd /caminho/do/data-contracts
git init && git add -A
git update-index --chmod=+x scripts/*.sh
git commit -m "chore: estrutura do repositorio de contratos"
git remote add origin <url> && git push -u origin main
```

⚠️ O `git update-index --chmod=+x` **não é opcional no Windows**: `core.fileMode`
é `false` lá, então `chmod` local nunca vira commit. Sem isso o runner recebe
`100644` e o job falha com *permission denied* — foi exatamente assim que o
primeiro release deste projeto reprovou.

## 3. Ajustar quem revisa

Abra `.github/CODEOWNERS` e troque `@sua-org/...` pelos times reais.

**Time que não existe não bloqueia nada** — é a falha mais silenciosa desse
arquivo, e o GitHub não avisa. Numa conta **pessoal** times não existem: use
usuários.

O que precisa ter dono distinto:

| Caminho | Dono | Por quê |
|---|---|---|
| `/contracts/<domínio>/` | time do domínio | é quem responde pelo dado |
| `/harness.lock` | plataforma + privacidade | mudar a versão reclassifica todos os contratos |
| `/.github/` | plataforma | o workflow não é derivado de nada e não dá para verificar por conteúdo |

## 4. Proteger a `main`

*Settings → Rules → Rulesets → New branch ruleset*, alvo `main`:

- **Require a pull request before merging**
- **Require review from Code Owners**
- **Dismiss stale pull request approvals when new commits are pushed** — crítico: o pedido de gate é identificado por hash de conteúdo, e sem isso uma aprovação carrega para um contrato diferente
- **Require status checks to pass** → marque `check` e `branch`
- **Deixe a lista de bypass vazia**

Sem isso, tudo acima é etiqueta. E note duas coisas que só aparecem no uso:

- O status check só aparece na lista **depois de ter rodado uma vez**. Abra um PR de teste antes de configurar.
- **Você não pode aprovar o próprio pull request.** Sozinho numa conta pessoal, exigir revisão significa nunca conseguir mergear o que você mesmo abre — para uma prova de conceito, exija só o status check.

## 5. O primeiro contrato

```bash
git checkout -b feat/202608/cadastro-clientes
mkdir -p contracts/clientes/cadastro
# escreva contracts/clientes/cadastro/contract.odcs.yaml
git push -u origin feat/202608/cadastro-clientes
```

O nível de domínio não é decoração: `contracts/<domínio>/<contrato>/` é o que
permite ao `CODEOWNERS` rotear a revisão. Sem ele, ou uma pessoa aprova todo
contrato da empresa, ou o arquivo cresce uma linha por contrato até ninguém mais
mantê-lo. O `check` emite **aviso** (não reprova) quando o contrato está raso.

Abra o pull request para `main`. Daí em diante o processo está em
[`processo.md`](processo.md).

---

## Manutenção

**Subir a versão do critério** é um pull request no repositório de contratos,
alterando `harness.lock`:

```bash
./scripts/novo-repo-de-contratos.sh /tmp/x v0.5.0   # só para ler o sha256 gerado
```

ou pegue o sha256 direto das notas do release. Subir de versão **pode
reclassificar campos sem que nenhum contrato mude** — é a alteração de maior
alcance possível naquele repositório, e por isso tem dono próprio.

**Atualizar o template** (workflow, scripts) é um commit aqui, em
`templates/repo-de-contratos/`, seguido de copiar o arquivo alterado para os
repositórios que já existem. Não há atualização automática, de propósito: um
repositório de contratos não deveria mudar de comportamento sem um commit
revisado.

## Portar para Azure DevOps

O template já traz `azure-pipelines.yml`, tradução direta do workflow do GitHub,
com os pontos de divergência marcados como `# DIVERGE`. O que muda:

| GitHub | Azure DevOps |
|---|---|
| `on: pull_request` | *branch policy* da branch de destino, configurada na UI |
| `gh release download` | `curl` (público) ou API autenticada / feed de artefatos |
| `contents: write` | permissão *Contribute* para a conta de build |
| `--formato github` | um `--formato azure` emitindo `##vso[task.logissue …]` — ainda não existe |
| `CODEOWNERS` | *Automatically included reviewers*, por caminho |
| `gh pr comment` | REST de *pull request threads* |

⚠️ **O `azure-pipelines.yml` ainda não foi exercitado num pipeline real.** Ele é
a tradução direta e revisada do workflow que roda, não uma execução comprovada.
Os detalhes de arquitetura da porta — inclusive os grupos do Entra ID como
aprovadores, e por que a separação entre quem aprova o contrato e quem aprova o
vocabulário é o risco real da migração — estão em
[`.github/README.md`](../.github/README.md#portar-para-azure-devops).
