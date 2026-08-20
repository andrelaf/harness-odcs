# O curso, e o que este projeto virou

Este projeto nasceu como trabalho final de um curso sobre **harness** — máquinas
que conduzem desenvolvimento incremental de forma determinística. O domínio
escolhido (classificação de privacidade de contratos ODCS) era, pelo brief, só o
"trabalho" que a máquina conduz.

Depois ele cresceu para o problema real de uma empresa. Este documento separa as
duas coisas: **o que o curso pediu e onde cada item está**, e **quanto do harness
foi de fato usado** — inclusive o que ficou subutilizado.

---

## 1. O pacote mínimo, item a item

O [`docs/brief.md`](brief.md) fecha com um checklist. Onde cada item está hoje:

| Item do checklist | Onde está | Estado |
|---|---|---|
| README curto: problema, restrições, arquitetura, como executar | [`README.md`](../README.md) | pronto |
| Ponto de entrada estável | [`run.sh`](../run.sh) — único, sem alternativa documentada | pronto |
| Estado e backlog persistidos | `state/feature-list.json`, `state/progress.json` | pronto |
| Smoke test e verificação com PASS/FAIL explícito | fases `smoke` e `verify`; `./run.sh doctor` usa o mesmo código | pronto |
| Handoff por feature: commit, resumo, testes, riscos | fase `handoff` + histórico do Git | pronto |
| Trace da trajetória | `trace/<run_id>.jsonl`, append-only | pronto |
| Medição: custo, duração, erros, resultado | `metrics/metrics.jsonl` · `./run.sh metrics` | pronto |
| Recomendação: quando usar e quando não | [`docs/decisao.md`](decisao.md) | pronto |

### Os cinco critérios essenciais

| Critério | Como este projeto responde |
|---|---|
| **Determinismo** | Ordem de fases explícita em `flow.rs`, teto de passos que **aborta** com exit `3`, e uma tabela de transições exercitada em `tests/flow.rs` (11 testes só disso). Um `FAIL` para no passo em que ocorreu — não tenta a próxima fase. |
| **Portabilidade** | A política vive em `run.sh` + binário compilado. Nada em `.cursorrules`, tasks de IDE ou config de editor. Prova adicional que o brief não pedia: a mesma política atravessou **GitHub Actions e um repositório separado**, sem reescrever regra. |
| **Verificação** | Nenhuma feature fecha sem `verify` PASS. Além disso, cada execução deixa `evidence/<run_id>/` com a saída bruta de toda ferramenta externa. |
| **Medição** | `./run.sh metrics` deriva de `trace/` — não há contador paralelo. 14 runs medidos: 81,5 s somados, **99,6% em espera de container**. A leitura honesta está em `decisao.md`. |
| **Decisão** | [`decisao.md`](decisao.md) diz onde se paga e, com o mesmo peso, onde uma alternativa simples vence. |

### O que faltava para fechar o checklist do curso

Dois itens do brief ficaram pendentes até depois da Semana 2, e hoje estão
prontos — vale saber que a pendência existiu, porque ela é parte da história:

**`BACKLOG-FUTURO.md`.** O brief da Semana 2 pede que o escopo seja congelado e
que tudo o que sobrar vá para esse arquivo. O escopo *foi* congelado na prática
— as 4 features não cresceram —, mas o arquivo que registra o que ficou de fora
só foi escrito depois. Está em [`BACKLOG-FUTURO.md`](../BACKLOG-FUTURO.md), e
desde então passou a ser onde as decisões de escopo são discutidas: é lá que
`sugerir` foi separado de `termos`, e onde ficou registrado que a seção de
atrito inteira espera um loop local que ninguém adotou.

**`docs/portabilidade.md`.** A Semana 2 pede prova registrada de que o mesmo
fluxo roda nas duas IDEs — o critério que o brief chama de "o grande item da
semana". A portabilidade sempre foi real (nada de IDE vazou para a política),
mas ficou um tempo **sem prova escrita**. Está em
[`portabilidade.md`](portabilidade.md).

O checklist do brief está fechado.

---

## 2. Quanto do harness foi realmente usado

Esta é a pergunta desconfortável, e a resposta honesta é: **o fluxo completo foi
usado para construir o domínio, e quase nada dele é usado para operar o que o
projeto virou.**

### O que foi usado de verdade

**As oito fases, quatro vezes.** F1, F2, F3 e F4 atravessaram
`start → plan → bearings → smoke → pick → implement → verify → handoff → stop`
inteiro, cada uma numa sessão fechada. Não é encenação: o `trace/` tem 14 runs,
com 4 `HALT` reais — fases que reprovaram e pararam o fluxo onde deviam.

**O gate humano (`implement → BLOCKED`).** Foi usado a sério: F4 travou em duas
lacunas, gravou `state/gate-pendente.json`, exigiu `./run.sh approve` e só então
escreveu no contrato. O hash das aprovações também provou seu valor — mudar o
contrato invalida a aprovação anterior.

**O teto de passos.** Disparou 2 vezes (exit `3`) durante o desenvolvimento.

**O trace e a evidência.** São a base da medição, e a medição mudou uma decisão
de projeto: descobrir que 99,6% do custo é partida de container é o que diz que
otimizar o Rust não paga.

### O que ficou subutilizado

**`state/progress.json` e a máquina de estados, depois que o domínio ficou
pronto.** O `check` — que é o que o CI roda hoje, e o que o processo real usa —
**não usa a máquina de estados**. Não avança feature, não conta passo, não fecha
ciclo.

> Este parágrafo dizia, até a divisão em crates, que o `check` *"carrega o
> estado só porque a struct `Run` exige, e nunca o salva"*. Era verdade, e era
> uma observação de prosa sobre uma coisa que o código permitia. Hoje o `check`
> monta um [`Ctx`](../crates/laudo/src/ctx.rs), que não tem onde guardar estado
> de fluxo, e as duas leituras de disco sumiram por impossibilidade.

Isso não foi acidente, foi necessidade: três pull requests abertos ao mesmo
tempo disputariam `state/progress.json`, e cada execução de CI viraria um commit
conflitando com os outros dois. A máquina de estados serve a **um** trabalho
sequencial conduzido por um agente; não serve a N verificações concorrentes.

**`plan`, `next`, `approve`, `reset` no dia a dia do produto.** Nenhum deles é
chamado pelo pipeline. O repositório de contratos usa `check`, `aplicar` e
`report` — e mais nada. Quatro dos nove comandos do harness existem só para o
ciclo de construção.

**A `feature-list`.** Congelada em 4 itens desde a Semana 1. Um harness pensado
para conduzir backlog vivo conduziu um backlog fechado.

**As duas IDEs.** Claude Code fez praticamente todo o trabalho; o VS Code foi
usado como execução manual do mesmo ponto de entrada. A portabilidade está
provada estruturalmente (nenhuma política em config de IDE), mas não foi
exercitada em volume.

### A leitura

O harness **fez o trabalho para o qual foi construído** — conduzir a construção
de quatro features com rastro, verificação e ponto de controle humano — e depois
a parte reutilizável dele não foi a máquina de estados: foram as **funções de
domínio puras** (`compor`, `classificar`, `ler_veredito`, `caminho_do_laudo`) e
a disciplina de evidência.

Dito de outro jeito: o harness foi um bom **andaime**. O que ficou de pé depois
que o andaime saiu foi o `check` — determinístico, sem estado, e chamável de
qualquer lugar.

**E isso deixou de ser uma leitura para virar a estrutura do repositório.** Onde
havia um crate, há dois:

```
crates/laudo/     o produto — contrato, glossário, catálogo, gate, laudo
crates/harness/   o andaime — fases, transições, progresso, medição
src/main.rs       o CLI, único lugar que importa dos dois
```

A dependência aponta `harness → laudo`, e **nunca** o contrário — não por
convenção: `laudo` não declara `harness` no `Cargo.toml`, então um `use
harness::` lá dentro não compila. A frase "o produto não precisa do andaime"
deixou de ser algo que este documento afirma e passou a ser algo que o
compilador recusa.

A costura entre os dois é um arquivo só,
[`harness/src/dispatch.rs`](../crates/harness/src/dispatch.rs): dado
`(feature, fase)`, qual função de domínio atende. É o único lugar do projeto que
precisa saber as duas coisas ao mesmo tempo — e o tamanho dele, quarenta linhas,
é a medida de quão pouco os dois lados realmente se tocam.

Isso não invalida o exercício; explica onde ele se paga. Um harness com máquina
de estados se paga enquanto há **trabalho sequencial a conduzir**. Quando o
trabalho vira **verificação concorrente**, o que sobrevive é a parte pura, e o
estado passa a atrapalhar.

---

## 3. Como apresentar isso

O [`docs/demo.md`](demo.md) tem o roteiro ensaiado, na ordem que o brief exige:
problema, execução com evidência, política atravessando ambientes, e fechamento
com medição e limites.

Uma sugestão de recorte para a banca, já que o projeto cresceu além do brief:
mostre o harness conduzindo uma feature (é o que está sendo avaliado) e use o
repositório de contratos apenas como **prova de portabilidade** — a mesma
política, sem reescrita, atravessando um segundo repositório e um CI.

E diga em voz alta o que está na seção 2. Reconhecer que a máquina de estados
ficou subutilizada no produto final é uma leitura mais forte do que fingir que
tudo foi usado — e é exatamente o tipo de honestidade que o `decisao.md` já
adota sobre os limites do classificador.
