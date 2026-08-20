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

Sem argumentos de versão, o script descobre o **último release de cada fluxo** —
a ferramenta e o vocabulário são publicados separados — e grava `harness.lock`
com as duas tags e os dois sha256. Para fixar outras:

```bash
./scripts/novo-repo-de-contratos.sh /caminho/do/data-contracts v0.8.0 vocab-v1.1.0
```

Ele filtra as tags por prefixo em vez de usar `/releases/latest`: os dois fluxos
dividem o espaço de tags do repositório, e aquele endpoint devolve o release mais
recente **qualquer que seja o fluxo** — um `vocab-v1.2.0` publicado hoje viraria
a "última versão" do binário, e o pin sairia apontando para um tarball que não
existe.

O que ele deixa lá:

```
.github/workflows/contrato-pr.yml   a esteira (GitHub Actions)
.github/CODEOWNERS                  quem revisa o quê
.github/pull_request_template.md    o que quem abre e quem aprova precisam dizer
azure-pipelines.yml                 a mesma esteira, para Azure DevOps
scripts/preparar.sh                 baixa os dois pacotes fixados (opcional, para dev)
scripts/verificar.sh                antecipa o veredito (opcional, para dev)
harness.lock                        os dois pins — gerado, não copiado
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

**Subir a versão do critério** é um pull request no repositório de contratos
alterando **duas linhas** do `harness.lock` — `HARNESS_VOCAB_VERSAO` e
`HARNESS_VOCAB_SHA256`, direto das notas do release do vocabulário.

É a alteração de maior alcance possível naquele repositório: **pode reclassificar
campos sem que nenhum contrato mude**. Por isso tem dono próprio, e por isso o
pipeline roda o `check` de todos os contratos com o vocabulário novo antes de
alguém aprovar — o relatório do pull request é a prévia do que a mudança faz.

Se a reclassificação contradisser o que um contrato declara, a F6 segura:
`aplicar` não sobrescreve a declaração humana, o item de gate aparece, e o merge
espera decisão. Enquanto esse pull request não entra, nenhum contrato muda de
veredito — que é o motivo de o pin existir.

**Subir a versão da ferramenta** é o mesmo movimento nas outras duas linhas, e
tem alcance menor: muda o código que julga, não o critério.

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
| `--formato github` | `--formato azure` — mesma anotação presa ao arquivo, sintaxe `##vso[task.logissue …]` |
| `CODEOWNERS` | *Automatically included reviewers*, por caminho |
| `gh pr comment` | REST de *pull request threads* |

⚠️ **O `azure-pipelines.yml` ainda não foi exercitado num pipeline real.** Ele é
a tradução direta e revisada do workflow que roda, não uma execução comprovada.
Os detalhes de arquitetura da porta — inclusive os grupos do Entra ID como
aprovadores, e por que a separação entre quem aprova o contrato e quem aprova o
vocabulário é o risco real da migração — estão em
[`.github/README.md`](../.github/README.md#portar-para-azure-devops).
