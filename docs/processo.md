# O processo, de ponta a ponta

Como um contrato de dados nasce, é classificado, revisado e arquivado — e quem
faz o quê em cada etapa.

Este documento cobre o processo *de produção*. O harness como máquina de
construção está em [`spec-harness.md`](spec-harness.md); o que o curso pediu, em
[`curso.md`](curso.md).

---

## Os três artefatos, e por que são três

| | O que é | Quem escreve | Onde vive |
|---|---|---|---|
| **harness-odcs** | O programa que julga, distribuído como pacote versionado | plataforma | este repositório |
| **vocabulário** | Glossário + catálogo LGPD — hoje dentro do pacote | privacidade / *data stewards* | este repositório, `glossary/` e `classification/` |
| **contratos** | Os contratos de dados e seus laudos | times de domínio | repositório separado |

**A separação é o controle.** O gate de privacidade só significa alguma coisa se
quem escreve o contrato não puder, no mesmo pull request, adicionar ao glossário
o termo que fecha a própria lacuna ou rebaixar no catálogo a classificação do
próprio campo. Deixar o vocabulário fora do repositório de contratos transforma
isso em **impossibilidade**, não em política de revisão bem preenchida.

O vocabulário viajar dentro do pacote é reversível e está documentado em
[`distribuicao.md`](distribuicao.md): `HARNESS_VOCAB` é a variável que o separa
no dia em que as duas cadências — código e vocabulário — começarem a doer.

---

## O fluxo de trabalho

![Da modelagem ao contrato classificado](fluxo.svg)

As três faixas do desenho são as três autoridades deste processo: quem **modela**
o dado, a **máquina** que julga sem opinar, e o **humano** que decide o que a
máquina não tem autoridade para decidir. Nada atravessa a segunda faixa sem
deixar rastro, e nada sai da terceira sem aprovação registrada.


```
pessoa escreve YAML  ─→  PR para main  ─→  esteira verifica e emite o laudo
                                              │
                                              ├─ FAIL  → autor corrige, novo commit
                                              └─ PASS/BLOQUEADO → revisão → merge
```

### 1. A pessoa escreve o contrato

```bash
git clone .../data-contracts && cd data-contracts
git checkout -b feat/202608/pedidos-online
# edita contracts/<dominio>/<contrato>/contract.odcs.yaml
git push -u origin feat/202608/pedidos-online
```

**Só isso.** Não precisa de Rust, não precisa de Docker, não precisa do pacote
de validação. Quem quiser antecipar o veredito pode rodar `./scripts/preparar.sh`
e `./scripts/verificar.sh`, mas é **opcional** — serve para encurtar o ciclo, não
para produzir nada.

O nome do diretório não é decoração: `contracts/<domínio>/<contrato>/` é o que
permite ao `CODEOWNERS` rotear a revisão para quem responde por aquele dado.

### 2. O pull request para `main` inicia tudo

Não há branch de longa duração além da `main`, e nada acontece por push direto.
A convenção de branch (`<tipo>/<aaaamm>/<descrição>`) está em
[`git-flow.md`](git-flow.md) e é verificada no CI.

### 3. A esteira verifica, emite e commita

Cinco passos, e nenhum deles decide nada:

1. **Baixa** o pacote na versão fixada em `harness.lock`
2. **Confere o sha256** — pacote diferente do fixado, para tudo
3. **Extrai** e prepara a imagem do motor, conferindo o digest
4. **Verifica** (`check`) — mesma política que roda em qualquer máquina
5. **Emite** o laudo (`aplicar`) e **commita na branch do PR**

O laudo é emitido **antes da revisão**, de propósito. Emitido depois, o commit
derrubaria a aprovação recém-dada (`dismiss stale approvals` prende a aprovação
ao SHA) e o mesmo conteúdo exigiria duas aprovações. Emitindo antes, quem aprova
vê de uma vez exatamente o que vai entrar.

Uma armadilha que custou uma rodada: **push feito com `GITHUB_TOKEN` não dispara
workflow**. É proteção do GitHub contra recursão, e derruba a suposição de que
"o commit novo será verificado pela execução seguinte" — ela não acontece. Por
isso a reverificação acontece no mesmo job, depois do commit.

### 4. O veredito

| Exit | Veredito | O que significa | O job |
|---|---|---|---|
| `0` | `pass` | Todo campo classificado, sem pendência | verde |
| `1` | `fail` | Contrato inválido, mal nomeado, ou lint reprovando | **vermelho** |
| `5` | `bloqueado` | Nada errado — falta decisão humana | **verde, com aviso** |

`5` não reprova, e é a escolha menos óbvia do desenho. Uma lacuna não é erro de
quem abriu o PR: é um campo que o glossário não cobre. Marcar vermelho diria à
pessoa errada que ela errou. Quem segura o merge nesse caso é a revisão de
CODEOWNER — a autoridade certa, e a que fica registrada no histórico.

Se reprovar, nada é emitido: sem proposta válida não há o que registrar.

### 5. A revisão

O `CODEOWNERS` chama quem responde pelo domínio. O revisor lê o comentário
automático — que traz veredito, resumo por campo e o laudo inteiro — e o diff,
que já contém o laudo commitado. **Uma aprovação, e o merge.**

---

## O git, na prática

**Uma branch por entrega, nascida da `main`.** `<tipo>/<aaaamm>/<descrição>`,
tipos `feat`, `fix`, `docs`, `chore`. Verificado no CI por
[`pr-convencao.yml`](../.github/workflows/pr-convencao.yml) — o único check que
reprova por processo em vez de conteúdo, e o único com regra em YAML, porque a
regra fala de git e o binário nunca vê um nome de branch.

**Merge commit, nunca squash nem rebase.** O histórico é parte do produto: o
gate é liberado por uma revisão aprovada, presa ao sha256 do contrato daquele
momento. Squash apaga a fronteira da entrega; rebase reescreve os SHAs que o
laudo referencia.

**O que protege cada coisa:**

| Caminho | Protegido por |
|---|---|
| `contracts/*/contract.odcs.yaml` | revisão de CODEOWNER do domínio |
| `contracts/*/laudos/` | **verificação de conteúdo** — o `check` recompara byte a byte; editar à mão reprova |
| `harness.lock` | CODEOWNER próprio (plataforma + privacidade): mudar a versão reclassifica todos os contratos |
| `.github/` | CODEOWNER da plataforma |
| glossário e catálogo | não estão no repositório |

Repare que `laudos/` **não depende de permissão**. Permissão se contorna com um
commit; verificação de conteúdo não, porque o conteúdo é determinado pelo
contrato e pelo critério.

⚠️ Nada disso vale sem **branch protection configurada**. Sem um ruleset na
`main` exigindo pull request e o status check `check`, o `CODEOWNERS` é
decoração — e times inexistentes nele são ignorados em silêncio.
