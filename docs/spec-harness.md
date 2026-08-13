# Spec do Harness — contrato congelado

> **Versão da spec:** 4 — ver [Mudanças](#12-mudanças)
> **Status:** congelada. Mudança aqui é decisão explícita, não efeito colateral de implementação.
> **Escopo:** o harness. Nada do classificador. A spec de cada feature de domínio (F1–F4) é escrita na sessão da própria feature.

Este documento existe por um motivo prático: o desenvolvimento acontece em sessões fechadas com `/clear` entre elas. O agente perde o contexto da conversa anterior. **Esta página é o que sobrevive ao `/clear`** — a sessão seguinte começa lendo daqui e implementa dentro deste contrato, em vez de reinventar nomes, códigos e formatos.

Fonte das restrições: [`contexto.md`](contexto.md). Fonte do plano semanal: [`brief.md`](brief.md). Onde houver conflito, aqueles dois vencem.

---

## 1. Princípio estrutural

**Política em Rust. Shell é burro.**

Todo `.sh` do repositório é um despachante: resolve caminho, invoca um processo, propaga o exit code. Zero lógica de negócio. Se um script ganhar um `if` de fluxo, virou uma segunda política — e o critério de portabilidade quebra na hora em que o fluxo rodar na segunda IDE.

Corolário: existe **um único** ponto de entrada, `run.sh`. Não há `run.ps1`, não há task de IDE, não há alias documentado como caminho alternativo. Dois pontos de entrada são duas políticas.

---

## 2. Ponto de entrada

```
./run.sh <comando> [flags]
```

`run.sh` garante o binário compilado, repassa `argv` e propaga o exit code. Nada além disso.

### Comandos

| Comando | Efeito | Muta estado? |
|---|---|---|
| `plan` | Cria/reconcilia `state/feature-list.json` com as 4 features na ordem F1→F4. Idempotente. | sim |
| `next` | Pega a primeira feature `pending` e a atravessa pelo fluxo inteiro até `Done` ou parada. | sim |
| `status` | Imprime feature corrente, fase, passo N/máx e último resultado. | não |
| `verify` | Re-executa apenas a fase `verify` da feature corrente. | sim (trace) |
| `handoff` | Re-executa apenas a fase `handoff` da feature corrente. | sim |
| `doctor` | Executa as checagens de ambiente e reporta PASS/FAIL por item. | não |
| `metrics` | Deriva custo, duração, erros e resultado de `trace/` e regenera `metrics/metrics.jsonl`. | não (só `metrics/`) |
| `approve <feature>` | Libera uma feature em `Blocked` e **arquiva o pedido de gate** que a bloqueou. | sim |
| `reset <feature>` | Devolve uma feature `Done` ou `Failed` para `Pending`, para reexecução. | sim |

Sobre `approve`: ele **não** decide nada de domínio. A feature que bloqueia
deixa o pedido em `state/gate-pendente.json`; `approve` o move para
`state/aprovacoes.json` com data e run, e apaga o pendente. A aprovação é
gravada pelo **hash do pedido**, não pelo nome da feature — vale para aquele
conjunto de itens, e nada mais. Mudou a entrada, o hash muda e o gate fecha
outra vez. O comando recusa liberar pedido que pertence a outra feature.

Sobre `reset`: uma feature concluída não tem como ser reexecutada sem editar
`state/feature-list.json` na mão — e editar estado na mão é exatamente o que o
harness existe para eliminar. O comando fecha esse buraco.

Ele **não** apaga trace nem evidência. O histórico de execuções anteriores é
imutável: reexecutar produz um novo `run_id`, e a comparação entre os dois é
justamente o que se quer poder auditar. `reset` também **não** libera feature
em `Blocked` — isso é atribuição de `approve`, e confundir os dois abriria um
caminho para contornar o gate humano sem aprovação.

### Flags globais

| Flag | Efeito |
|---|---|
| `--step` | Avança **uma** transição em vez da feature inteira. |
| `--dry-run` | Imprime a sequência de transições que seria executada e sai. Não toca disco. |
| `--json` | Saída legível por máquina, em stdout. **Não implementado** — ver [Mudanças, v4](#12-mudanças). |

### Exit codes

O contrato de saída é parte da spec — é o que torna o determinismo verificável de fora.

| Código | Significado |
|---|---|
| `0` | PASS |
| `1` | FAIL de fase |
| `2` | Uso incorreto (comando desconhecido, argumento faltando) |
| `3` | Teto de passos atingido — abortado |
| `4` | Estado ausente, ilegível ou com `schema_version` incompatível |
| `5` | Bloqueado aguardando decisão humana |

Regra: mensagem de erro vai para **stderr**; saída de dados vai para **stdout**. `--json` nunca contamina stdout com log.

---

## 3. Fases

Nove fases, espelhando literalmente o fluxo canônico do brief:

```
start → plan → bearings → smoke → pick → implement → verify → handoff → stop
```

```rust
pub enum Phase { Start, Plan, Bearings, Smoke, Pick, Implement, Verify, Handoff, Stop }
```

`Start` e `Stop` são variantes reais, não bordas implícitas. O critério avaliado é "ordem explícita" — o enum precisa poder ser lido lado a lado com o brief sem interpretação.

| Fase | Responsabilidade | Hook de domínio? |
|---|---|---|
| `Start` | Abre `run_id`, cria o arquivo de trace, carrega e valida o estado. | não |
| `Plan` | Confirma que há feature elegível e registra a intenção do run. | não |
| `Bearings` | Lê onde o trabalho parou: estado, branch, último handoff. | não |
| `Smoke` | Preflight de ferramentas: engine de container acessível, imagem presente, git utilizável, `schema_version` compatível. | não |
| `Pick` | Marca a feature escolhida como `InProgress`. Aplica o invariante de exclusividade. | não |
| `Implement` | Executa o trabalho da feature. | **sim** |
| `Verify` | Produz evidência de que o trabalho está correto, com PASS/FAIL explícito. | **sim** |
| `Handoff` | Commita, registra resumo, testes e riscos. Marca a feature como `Done`. | sim (resumo) |
| `Stop` | Fecha o trace e persiste o estado final. | não |

Resolução do hook, nesta ordem: `features/<feature-id>/<fase>` → implementação genérica → no-op `Pass`. É isso que permite adicionar F1 sem tocar no núcleo do fluxo.

---

## 4. Transições

O núcleo é uma **função pura**, sem I/O:

```rust
fn decide(current: Phase, outcome: Outcome, step: u32, max_steps: u32) -> Transition
```

```rust
pub enum Outcome { Pass, Fail(Reason), Blocked(Reason) }

pub enum Transition { Advance(Phase), Halt(HaltReason), Complete }

pub enum HaltReason {
    PhaseFailed(Phase, Reason),
    StepCeiling { at: Phase, max: u32 },
    AwaitingHuman(Phase, Reason),
}
```

Ser pura não é preciosismo: é o que permite a `tests/flow.rs` enumerar a tabela inteira sem disco, sem Docker e sem git. "Determinismo testável" (critério da Semana 3) depende disso.

### Tabela de decisão

Avaliada **nesta ordem**, primeira regra que casa vence:

| # | Condição | Resultado | Exit |
|---|---|---|---|
| 1 | `step >= max_steps` | `Halt(StepCeiling)` | `3` |
| 2 | `outcome = Blocked(r)` | `Halt(AwaitingHuman)` | `5` |
| 3 | `outcome = Fail(r)` | `Halt(PhaseFailed)` | `1` |
| 4 | `outcome = Pass` e `current = Stop` | `Complete` | `0` |
| 5 | `outcome = Pass` | `Advance(próxima fase)` | — |

O teto é verificado **dentro** de `decide`, regra 1. Espalhar essa checagem pelo laço de execução a tornaria não testável — e o brief exige demonstrá-la.

Não existe transição para trás, não existe pulo de fase e não existe caminho condicional entre fases. A ordem é a do array. Um `Fail` **para** — não tenta a próxima fase.

---

## 5. Estado

Quatro arquivos em `state/`, todos versionados no Git. Estado é evidência, não artefato de build.

Os dois primeiros existem sempre; os dois do gate só existem quando há gate:

| Arquivo | Quando existe |
|---|---|
| `feature-list.json` | sempre |
| `progress.json` | sempre |
| `gate-pendente.json` | entre o `Blocked` de uma feature e o `approve` que o consome |
| `aprovacoes.json` | a partir da primeira aprovação; append-only |

Escrita **atômica** obrigatória: escreve em temporário, depois renomeia. Crash no meio da escrita não pode deixar estado corrompido — e o exit `4` existe exatamente para o caso em que deixou.

### `state/feature-list.json`

```json
{
  "schema_version": 1,
  "features": [
    { "id": "f1-validar",     "order": 1, "title": "Validar contrato contra o schema ODCS", "status": "pending" },
    { "id": "f2-mapear",      "order": 2, "title": "Mapear campos ao glossário canônico",   "status": "pending" },
    { "id": "f3-classificar", "order": 3, "title": "Classificar PII/LGPD por campo",        "status": "pending" },
    { "id": "f4-gate",        "order": 4, "title": "Gate humano + relatório de lacunas",    "status": "pending" }
  ]
}
```

```rust
pub enum FeatureStatus { Pending, InProgress, Blocked, Done, Failed }
```

A ordem é explícita no campo `order`. Nunca inferida de posição no array nem de ordenação de string.

### `state/progress.json`

```json
{
  "schema_version": 1,
  "run_id": "20260811T232400Z-a1b2c3",
  "current_feature": "f1-validar",
  "current_phase": "smoke",
  "step_count": 3,
  "max_steps": 12,
  "run_status": "running",
  "last_result": "PASS",
  "last_transition_at": "2026-08-11T23:24:07Z",
  "attempts": 1
}
```

`run_status` ∈ `idle` | `running` | `blocked_on_human` | `failed` | `done`.

`idle` é o estado antes do primeiro run e depois de um `approve`. Sem ele, um
`progress.json` recém-criado teria de mentir sobre estar em algum dos outros
quatro.

`max_steps` mora **no arquivo**, não em constante compilada nem em variável de ambiente. A Semana 3 precisa provar o abort por teto; o teto tem que ser inspecionável e ajustável sem recompilar.

### `state/gate-pendente.json` e `state/aprovacoes.json`

```json
{ "schema_version": 1, "feature": "f4-gate",
  "gate_sha256": "4f773bab…", "run_id": "20260813T031348Z-11391c",
  "criado_em": "2026-08-13T03:13:51Z",
  "resumo": "2 lacuna(s), 0 reclassificacao(oes), 0 conflito(s)",
  "itens": ["[lacuna] segmento — …"] }
```

```json
{ "schema_version": 1, "aprovacoes": [
  { "feature": "f4-gate", "gate_sha256": "4f773bab…",
    "aprovado_em": "2026-08-13T03:14:02Z", "run_id": "…", "resumo": "…" } ] }
```

O `gate_sha256` é a identidade do **conteúdo** submetido, calculada pela
feature. O harness não sabe o que ele significa — só que a aprovação vale para
aquele hash e para aquela feature. É o que impede uma aprovação de virar passe
permanente.

`itens` são as linhas que a pessoa lê para decidir. Quem aprova não aprova um
hash.

### Invariantes

Verificados ao carregar o estado. Violação → exit `4`.

1. No máximo **uma** feature em `InProgress`. É o "uma sessão, uma feature" do brief virando trava executável.
2. `step_count <= max_steps`.
3. `schema_version` conhecido.
4. `current_feature`, quando presente, existe em `feature-list.json`.
5. Uma feature `Blocked` só sai desse estado por `approve` — nunca por um `next` subsequente.

---

## 6. Trace

`trace/<run_id>.jsonl` — append-only, um evento JSON por linha.

**JSONL e não JSON** por dois motivos: append é seguro sob interrupção, e a leitura não depende de ferramenta externa (`jq` não está disponível no ambiente alvo).

```json
{"ts":"2026-08-11T23:24:07Z","run_id":"20260811T232400Z-a1b2c3","seq":4,
 "feature":"f1-validar","from":"bearings","to":"smoke","event":"phase_end",
 "result":"PASS","duration_ms":142,"exit_code":0,"step":3,"msg":""}
```

### Eventos

| `event` | Quando |
|---|---|
| `run_start` | Abertura do run, em `Start` |
| `phase_start` | Entrada em cada fase |
| `phase_end` | Saída de cada fase, com `result` e `duration_ms` |
| `tool_exec` | Toda invocação de processo externo |
| `blocked` | Fase retornou `Blocked` |
| `abort` | Teto de passos atingido |
| `run_end` | Fechamento do run, em `Stop` |

`seq` é monotônico dentro do run. `ts` em UTC, ISO-8601.

### `duration_ms` e `exit_code` desde já

A Semana 3 pede custo, duração, número de erros e resultado por execução. Com esses campos presentes desde a Semana 1, `metrics.jsonl` vira uma **derivação** do trace — não uma segunda instrumentação paralela que pode divergir.

Cumprido: `./run.sh metrics` lê `trace/*.jsonl` e regenera `metrics/metrics.jsonl` inteiro. Nenhum contador novo é escrito durante a execução, não há estado incremental para corromper, e apagar `metrics/` não perde nada.

Duas leituras que a derivação obriga a separar:

- **Erro é fase reprovada**, não exit code diferente de zero. O fluxo sonda o repositório com `git rev-parse --verify` para saber se a branch existe, e o exit `1` dessa sondagem é a resposta "não existe". Somar as duas coisas inflaria a contagem de erros com o funcionamento normal — por isso `erros` e `ferramentas_nao_zero` são campos distintos.
- **Run sem `run_end` é `INCOMPLETO`**, não `FAIL`. Interrompido por fora não é reprovado, e tratar como falha erraria justamente o número que o brief pede.

### Trave de privacidade

O trace é o artefato que circula. Portanto:

- **Nenhum valor de campo do contrato entra no trace.** Só nomes de campo, decisões e justificativas.
- A saída bruta de ferramentas externas vai para `evidence/<run_id>/`; o trace guarda **referência + hash**, nunca o conteúdo.
- Nada de variáveis de ambiente, tokens ou credenciais em nenhum evento.

Isso implementa na camada de artefato a restrição do `contexto.md` de que o agente atua apenas sobre metadados.

---

## 7. Fronteira com o mundo externo

Nenhuma chamada de processo espalhada pelo código. **Um** `ToolRunner` executa tudo e registra `tool_exec` com: comando, argumentos, exit code, duração, e a **tag/digest da imagem** quando for container.

A imagem do `datacontract-cli` é **fixada**, nunca `:latest`. `latest` destrói reprodutibilidade — o mesmo contrato pode passar hoje e falhar amanhã sem nenhum commit no repositório. A versão fixada vive em `scripts/env.sh` e é registrada em cada run.

Pin vigente, verificado em 12/08/2026:

```
DC_IMAGE=datacontract/cli:1.1.0
DC_DIGEST=sha256:f7fa02d649f4992dd8297bb428ece7403d688e881cf4a386673e250cb678657b
```

O `smoke` compara o digest da imagem local com `DC_DIGEST` e falha se divergir. Trocar de versão é uma decisão registrada, não um efeito de `docker pull`.

---

## 8. Git

- O harness **não escreve na `main`**. `next` recusa executar com `HEAD` em `main` sem branch de trabalho.
- Branch por feature: `feat/<feature-id>`.
- O commit é feito pela fase `handoff` e **anunciado no output** — o operador vê o hash sem precisar procurar.
- Escopo do commit: `state/`, `trace/`, `evidence/` e `contracts/`. Nunca a árvore inteira. `contracts/` está na lista porque o contrato enriquecido é o entregável, e o diff dele é o que vai para revisão.
- Nenhum segredo é persistido em artefato versionado.

---

## 9. Ambiente

Scripts em `scripts/`, todos despachantes:

| Arquivo | Papel |
|---|---|
| `env.sh` | Fonte única: raiz do repo, sufixo `.exe`, caminho do binário, engine de container, tag fixada da imagem. Apenas *sourced*. |
| `bootstrap.sh` | Idempotente: valida toolchain, cria `state/ trace/ evidence/`, faz `pull` da imagem fixada. |
| `doctor.sh` | Wrapper de `run.sh doctor`. |
| `datacontract.sh` | Wrapper do container com o mount padrão. |
| `editor.sh` | Sobe o editor ODCS local. |
| `dev.sh` | `fmt` + `clippy` + `test`. |
| `ci.sh` | `bootstrap` → `dev` → `doctor`. O mesmo comando roda local e no CI. |

As checagens de `doctor` e da fase `smoke` são **o mesmo código**, chamado de dois lugares. Duas listas de checagem divergem em duas semanas.

---

## 10. Fora de escopo desta spec

Não pertencem aqui e não devem ser adicionados: glossário canônico, regras de detecção de PII, formato do relatório de lacunas, schema ODCS, qualquer decisão sobre o classificador. Isso é domínio, vive nas features, e cada uma traz sua própria spec curta na sua sessão.

Ideias novas de qualquer natureza vão para `BACKLOG-FUTURO.md` — não entram no projeto.

---

## 12. Mudanças

Registro das decisões que alteraram este contrato depois de congelado.

### Versão 4 — 13/08/2026

**Adicionado `metrics`** (seções 2 e 6). Não é mudança de comportamento do
fluxo: é a materialização do que a seção 6 já prometia ao exigir `duration_ms`
e `exit_code` desde a Semana 1.

Escopo estreito de propósito: o comando **só lê** `trace/` e escreve
`metrics/metrics.jsonl`. Não toca em `state/`, não participa do fluxo e não é
chamado por nenhuma fase. Instrumentar a medição dentro das fases criaria a
segunda fonte de verdade que a seção 6 existe para evitar.

**Registrado: `--json` está declarado e não implementado** (seção 2). Apareceu
na conferência de fechamento, comparando a tabela de flags com o `Cli` de
`main.rs`: só `--step` e `--dry-run` existem. É o mesmo tipo de divergência que
a versão 2 corrigiu em `run_status`, e a correção honesta é a mesma — dizer, em
vez de deixar a spec prometer o que o binário não entrega. Implementar é
decisão para uma sessão própria, não efeito colateral do empacotamento.

### Versão 3 — 13/08/2026

**`approve` passou a arquivar o pedido de gate** (seções 2 e 5), com dois
arquivos novos em `state/`: `gate-pendente.json` e `aprovacoes.json`.

Motivo: na v2, `approve` só devolvia a feature para `Pending`. Implementando F4
isso se mostrou insuficiente — a feature bloqueia recomputando a mesma
divergência a cada run, então voltar para `Pending` fazia o `next` seguinte
bloquear de novo, em laço. Faltava o registro de *o quê* foi aprovado.

A alternativa descartada foi carimbar a aprovação na feature ("f4 está
liberada"). Seria um passe permanente: aprovar uma lacuna hoje liberaria em
silêncio uma despromoção de campo PII amanhã. A aprovação vale para um
conteúdo, identificado por hash — e o hash é calculado pela feature, não pelo
núcleo, que continua sem saber o que ele significa.

`approve` segue burro: não recomputa nada, só move o pedido para o livro.

**Escopo do commit do `handoff` ganhou `contracts/`** (seção 8). O contrato
enriquecido é o entregável do projeto e F4 o escreve ali depois do veredito;
fora da lista, o commit registraria a decisão em `evidence/` e não o efeito
dela.

### Versão 2 — 12/08/2026

**Adicionado `reset <feature>`** (seção 2). Implementação prevista para a sessão
de F1.

Motivo: ao concluir F1 com `verify` ainda em no-op, a feature ficou `Done`. Para
reexecutá-la com a validação real, a única saída era editar
`state/feature-list.json` na mão — contradizendo o princípio de que o estado é
manipulado pelo ponto de entrada, nunca por edição direta. A lacuna apareceu no
uso, não no desenho.

Escopo deliberadamente estreito: só transita `Done`/`Failed` → `Pending`. Não
toca em trace nem evidência, e não substitui `approve`.

**Corrigido `run_status`** (seção 5): a lista omitia `idle`, que a implementação
já usa como estado inicial. Divergência entre spec e código, não mudança de
comportamento.

### Versão 1 — 12/08/2026

Contrato inicial: comandos, exit codes, as 9 fases, tabela de decisão, schemas
de estado e trace, invariantes, fronteira com ferramentas externas e regras de
Git.
