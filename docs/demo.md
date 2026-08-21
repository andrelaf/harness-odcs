# Roteiro de demo

> **Ensaiado.** Toda saída abaixo foi produzida por execução real e está em
> `trace/` e `evidence/`. A ordem é a exigida pelo brief: abre pelo problema,
> fecha pela recomendação.
>
> **Duração alvo: 10 minutos.** Os tempos por bloco são folgados de propósito —
> o fluxo leva ~6 s e a tentação é preencher o silêncio explicando código.

---

## Antes de começar

```bash
./run.sh doctor        # 6 PASS. Se algo reprovar, resolva antes — não improvise na frente da banca.
git status --short     # tem de estar limpo
```

Tenha aberto: um terminal, o `README.md` e uma aba em `evidence/`.

**Não abra o código-fonte durante a demo.** Se perguntarem de implementação, o
lugar é `docs/spec-harness.md`.

---

## 1 · O problema e as restrições — 2 min

*Não comece pela solução.*

Uma organização acumula contratos de dados. Cada campo precisa de resposta para
uma pergunta chata: **isso é dado pessoal?** Na mão não escala e não deixa
rastro. Três problemas concretos:

- **Cobertura** — um contrato de 40 campos revisado por uma pessoa cansada tem
  campo esquecido, e ninguém percebe até a auditoria.
- **Justificativa** — "esse campo é PII" sem o porquê registrado não serve para
  compliance.
- **Consistência** — `cpf`, `nr_cpf` e `documento` são a mesma coisa;
  classificados por pessoas diferentes, viram três respostas.

Jogar um LLM no problema resolve a velocidade e piora todo o resto: esquece
campos, reprocessa o que já fez, encerra com relatório incompleto e classifica
sem justificar.

> **A frase que abre a demo:** *"Por isso o entregável aqui é o harness, e não o
> classificador. O modelo não decide nada neste fluxo — e é disso que vêm as
> garantias."*

As restrições que moldaram tudo (de `docs/contexto.md`): dados sintéticos
apenas; o agente atua sobre metadados, nunca sobre os dados; reclassificação
sensível exige aprovação humana; sem escrita direta na `main`; nada rodando
permanentemente.

---

## 2 · Uma execução curta + a evidência — 3 min

Mostre o estado e o teto:

```bash
./run.sh status
```

As quatro features `done`, `passos 9/12`. **Aponte o teto**: 12 passos, e ele
mora em `state/progress.json`, não em constante compilada.

Agora reexecute a última feature de ponta a ponta:

```bash
./run.sh reset f4-gate
./run.sh next
```

Nove fases, `start → … → stop`, cada uma com PASS explícito. **Fale duas
linhas enquanto roda**, não mais:

- `smoke` compara o digest da imagem do `datacontract-cli` com o pin do
  repositório — versão fixada, nunca `latest`.
- `verify` **refaz o trabalho do zero** e compara byte a byte com o que
  `implement` propôs. Divergência só pode significar entrada alterada no meio
  do run.

A evidência, que é o ponto:

```bash
cat evidence/<run_id>/f4-relatorio.md
```

Mostre a tabela: cada campo com termo do glossário, classificação, `pii`,
referência legal e justificativa. **Nove campos, nenhum sem decisão.**

Uma linha que vale dizer em voz alta: *"`segmento` e `valor_total_compras`
saíram como lacuna. O harness não adivinhou — nomeou o que não sabia."*

---

## 3 · O ponto de controle humano — 3 min

*Este é o bloco que decide a demo. Não corte por tempo.*

Simule o que acontece na vida real: o encarregado de dados reclassifica um termo
no catálogo. Edite `classification/catalogo-lgpd.yaml`:

```yaml
version: 2.0.0                    # major: reclassificação de termo já classificado

  - termo: cadastro.data_criacao
    classification: confidential  # era internal
    pii: true                     # era false
```

```bash
./run.sh reset f4-gate
./run.sh next; echo "exit=$?"
```

Saída real:

```
  implement  BLOCKED
             catalogo classification/catalogo-lgpd.yaml v2.0.0 — 8 termo(s) classificado(s)
               gate — [reclassificacao] data_cadastro — o contrato declara `internal, nao pii` e o catalogo diz `confidential, pii`
               gate — [lacuna] segmento — campo sem termo no glossario …
               gate — [lacuna] valor_total_compras — campo sem termo no glossario …
             pedido 4ada9f547c62f9f0 registrado em state/gate-pendente.json
bloqueado em `implement` aguardando decisao humana: 2 lacuna(s), 1 reclassificacao(oes)
exit=5
```

Três coisas para apontar, nesta ordem:

1. **O contrato não foi tocado.** `git diff` está vazio. O fluxo parou antes de
   persistir.
2. **Ninguém avisou o harness da mudança.** Ele leu o que o contrato declara
   hoje e comparou com o que o catálogo diz agora. A divergência apareceu
   sozinha.
3. **O pedido tem hash `4ada9f54`, e a aprovação anterior era `4f773bab`.** A
   lacuna aprovada na semana passada **não liberou** esta reclassificação. A
   aprovação vale para um conteúdo, nunca para uma feature.

O item 3 é a resposta para *"e se alguém aprovar uma vez e virar carta branca?"*.
Tenha `state/aprovacoes.json` pronto para abrir.

```bash
./run.sh approve f4-gate      # imprime o que está sendo aprovado, item a item
./run.sh next                 # agora atravessa e escreve o contrato
git diff contracts/           # o efeito: classification, tags e authoritativeDefinitions
```

O diff em `contracts/` é o entregável indo para revisão humana como qualquer
outra mudança de código.

### A mesma política nas duas IDEs

Trinta segundos, sem teatro:

```bash
ls -a | grep -i vscode        # não existe
ls scripts/                   # despachantes, zero lógica de fluxo
```

Não há `.vscode/`, `tasks.json`, `run.ps1` nem alias documentado. **Um ponto de
entrada, `./run.sh`**, e a política inteira compilada em Rust. Rodar do terminal
integrado do VS Code ou do Claude Code produz a mesma saída e o mesmo exit code
porque não há caminho alternativo para produzir outra. O workflow do CI chama
`./run.sh` — o mesmo ponto de entrada, sem sequência própria.

---

## 4 · Medição, limites e recomendação — 2 min

```bash
./run.sh metrics
```

Números reais, 14 execuções:

| | |
|---|---|
| Runs | 14 — 10 `PASS`, 4 `HALT` |
| Duração somada | 81,5 s |
| Em ferramenta externa | **99,6%**, em 153 invocações |
| Erros | 1 fase reprovada · 2 bloqueios · 2 abortos |

**A leitura, que vale mais que a tabela:** 99,6% do tempo é espera de container,
a ~530 ms por invocação. Otimizar o harness não paga — o custo escala com
invocações, não com número de campos. E não há custo de token: nenhum modelo
roda no fluxo.

Mostre o bloco `onde travou`. Os **três desfechos de parada previstos na spec
aconteceram de verdade**: teto de passos (exit 3), fase reprovada (exit 1) e
gate humano (exit 5). O teto não é demonstrável só em teste — está no registro,
no run `ad8cec`, que rodou com `max_steps: 4` e abortou em `smoke`.

Feche com os limites, sem suavizar:

- **22% dos campos saíram como lacuna** no primeiro contrato real. A cobertura é
  do glossário, não da ferramenta.
- Um contrato, sintético, plano — propriedades aninhadas não foram exercitadas.
- A reescrita do YAML perde comentários.
- Mil contratos seriam ~1h40 só de partida de Docker.

E a recomendação (`docs/decisao.md`):

> **Use** com volume de contratos, exigência de rastreabilidade e um dono para o
> vocabulário. **Não use** para um contrato único e trivial — revisar 9 campos
> na mão leva 15 minutos, e isto custou 4 semanas.
>
> A pergunta que separa os dois casos não é *"quantos contratos você tem?"*, é
> **"alguém vai ter que provar, depois, por que este campo foi classificado
> assim?"**.

---

## Depois da demo

```bash
git checkout -- classification/ contracts/ state/
rm -f state/gate-pendente.json
./run.sh status                      # 4 features done
```

`trace/` e `evidence/` **não** são apagados: o histórico é imutável, e as
execuções da demo são evidência tão legítima quanto as outras. Só o cursor de
estado volta ao lugar.

A diferença entre os dois é o que o repositório guarda. `trace/` é versionado —
é dele que a medição sai, e é o entregável que o brief nomeia. `evidence/` fica
no disco e é ignorada pelo `.gitignore`, porque é regenerável a partir do
contrato e do critério, ambos fixados por versão. Rodar a demo não deixa
nenhum arquivo para commitar depois.

---

## Perguntas prováveis, e a resposta curta

| Pergunta | Resposta |
|---|---|
| *"Onde está o agente/LLM?"* | Em lugar nenhum dentro do fluxo. A classificação é consulta a catálogo. O agente escreveu o harness; ele não opera dentro dele. |
| *"E se o catálogo estiver errado?"* | O harness não julga a lei. Ele garante que a decisão veio do catálogo, com referência legal, e que mudá-la passa pelo gate. |
| *"Por que não usar um LLM para classificar?"* | Classificaria mais rápido e perderia cobertura verificável, justificativa por decisão e controle do que é persistido. Se essas garantias não forem requisito, um LLM é a escolha certa e isto é caro demais. |
| *"Isso escala?"* | Até a partida do container. 530 ms × 11 invocações por run. Acima de algumas centenas de contratos, precisa de lote ou CLI de vida longa. |
| *"O que acontece se o agente esquecer um campo?"* | Não é possível esquecer sem reprovar: `verify` confere que cada campo do contrato aparece exatamente uma vez, e campo a mais ou a menos é FAIL. |
| *"Por que Rust?"* | Porque a política precisa ser um binário único, testável sem disco nem container, e igual nas duas IDEs. `crates/harness/tests/flow.rs` enumera a tabela de transições inteira sem I/O. |
