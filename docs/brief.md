# Brief — Harness sob Restrição (Projeto Final)

> Documento de trabalho para conduzir o projeto no Claude Code, sessão a sessão.
> Turma fundadora · Demo Day: **10/09/2026, 19h30 BRT**.

---

## 0. Enquadramento (leia antes de tudo)

**O entregável é o harness, não o classificador.** O domínio real — um classificador de privacidade de contratos ODCS — é só o "trabalho" que o harness conduz. O que está sendo avaliado é a máquina de orquestração: um fluxo determinístico que dirige o desenvolvimento incremental, roda igual em **duas IDEs**, e prova cada passo por evidência.

O fluxo canônico do harness:

```
start → plan → bearings → smoke → pick → implement → verify → handoff → stop
```

Cada **feature** do domínio atravessa esse fluxo inteiro, uma por vez, com teto de passos e handoff registrado em Git.

### Domínio congelado (não crescer)

O classificador é entregue como 4 features pequenas. Este escopo está **fechado** — qualquer ideia nova vai para um `BACKLOG-FUTURO.md` e não entra no projeto:

- **F1 — Validar:** o contrato ODCS é válido contra o schema.
- **F2 — Mapear:** cada campo é casado com um glossário canônico.
- **F3 — Classificar:** cada campo recebe classificação PII/LGPD.
- **F4 — Gate + relatório:** lacunas apontadas e reclassificação sensível pausada para aprovação humana.

O risco número um do projeto é o classificador crescer e engolir o tempo do harness. Se em qualquer semana você estiver "melhorando o classificador" em vez de "fortalecer o harness", pare.

### Restrições preservadas (registrar no README)

Domínio sintético; nenhum dado real ou de produção. O agente atua só sobre metadados do contrato, nunca sobre os dados. Sem rede externa além do necessário. Sem escrita direta na main. Nenhum segredo persistido em artefato.

---

## 1. Decisões a tomar já (Semana 1, dia 1)

Duas travas condicionam todo o resto. Decida antes de escrever qualquer código.

**Quais duas IDEs.** A portabilidade é critério essencial. A regra de ouro: **a política do workflow não pode morar em config de IDE.** Ela vive no ponto de entrada (`run.sh`) e nos arquivos de estado — nunca em `.cursorrules`, tasks do VS Code ou equivalente. Assim o mesmo fluxo roda em qualquer terminal. Recomendação segura: **VS Code + Cursor** (ambos com terminal e agente); alternativa: VS Code + Zed. Escolha e não mude mais.

**Congelar o ponto de entrada.** `run.sh` recebe um comando (`plan`, `next`, `verify`, `handoff`, `status`) e é o único jeito de operar o harness. Nada de rodar passos soltos à mão.

---

## Semana 1 — Domínio, brief, fluxo mínimo e primeiro trace

**Objetivo:** o harness roda **uma** feature de ponta a ponta e gera o primeiro rastro. Não é implementar o classificador inteiro — é provar que a máquina anda.

**Entregas:**
- `README.md` curto (esqueleto): problema, restrições, como executar.
- `run.sh` estável com pelo menos `plan`, `next`, `status`.
- Estado persistido: `state/feature-list.json` (as 4 features + status) e `state/progress.json`.
- Um `trace/` que registra cada transição do fluxo com timestamp.
- **F1 (Validar)** atravessando `pick → implement → verify → handoff` com um commit real.

**Critérios que ataca:** Determinismo (ordem explícita), primeiro sinal de Verificação.

**Definition of Done:** rodar `./run.sh next` executa F1 inteira, termina com PASS/FAIL explícito e deixa um commit + uma entrada de trace. Se falhar, para no passo certo — não avança.

**Levar pro Claude Code:** uma sessão só para o esqueleto do harness (run.sh + estado + trace) e uma sessão separada para F1. Um `/clear` entre elas.

---

## Semana 2 — Skill adaptada, segunda IDE e escopo congelado

**Objetivo:** o mesmo fluxo passa a rodar na **segunda IDE** sem reescrever a política, e o domínio ganha corpo com F2 e F3.

**Entregas:**
- Prova de portabilidade: o mesmo `run.sh` dirigindo o fluxo nas duas IDEs (registre como — screenshots, trace de ambas, ou um `docs/portabilidade.md`).
- **F2 (Mapear)** e **F3 (Classificar)** implementadas via harness, cada uma com seu handoff.
- Skill/instrução do agente ajustada ao domínio (o "como classificar", o glossário, as regras determinísticas de PII conhecida como CPF e e-mail).
- **Escopo congelado** ao fim da semana: `BACKLOG-FUTURO.md` recebe tudo que sobrar.

**Critérios que ataca:** Portabilidade (o grande item da semana), mais Verificação por feature.

**Definition of Done:** um observador roda o mesmo ponto de entrada nas duas IDEs e obtém o mesmo comportamento. F2 e F3 concluídas com evidência.

**Levar pro Claude Code:** uma feature por sessão. Ao testar a segunda IDE, confirme que nada específico de IDE vazou para dentro da lógica do fluxo.

---

## Semana 3 — Smoke, verify, handoff e primeira medição

**Objetivo:** fechar o loop de qualidade (smoke + verify robustos) e começar a **medir de verdade**.

**Entregas:**
- Smoke test com PASS/FAIL explícito antes de cada implementação.
- **F4 (Gate + relatório)** implementada: lacunas apontadas, reclassificação sensível pausa para humano.
- Handoff completo por feature: commit, resumo, testes e riscos.
- **Medição:** captura real de custo, duração, número de erros e resultado por execução (`metrics/` ou um `metrics.jsonl`).
- Teste de determinismo: prova de que o fluxo respeita a ordem e **aborta ao bater o teto de passos**.

**Critérios que ataca:** Medição (item mais fraco hoje — priorize), Verificação, Determinismo testável.

**Definition of Done:** existe número real de custo/duração e uma leitura honesta dos limites — onde travou, onde saiu caro. O teto de passos é demonstrável.

**Levar pro Claude Code:** peça ao agente para instrumentar a medição como uma feature própria, não como enfeite. A leitura honesta dos limites é o que a banca valoriza.

---

## Semana 4 — Versão apresentável, README de decisão e demo

**Objetivo:** empacotar, escrever o README de decisão e ensaiar o Demo Day.

**Entregas:**
- README final completo: problema, restrições, arquitetura, como executar.
- **Seção de decisão:** onde o harness se paga e onde uma alternativa simples vence. Para este domínio, algo como: *vale quando há muitos contratos e compliance exige rastro; não vale para um contrato único e trivial, onde revisar na mão é mais rápido.*
- Recomendação de uso (quando usar / quando não usar).
- Trace e métricas consolidados como evidência.
- Roteiro de demo ensaiado (ver abaixo).

**Critérios que ataca:** Decisão, e o fechamento de todos os outros.

**Definition of Done:** o pacote mínimo do material está 100% marcado e a demo cabe no tempo.

---

## Roteiro do Demo Day (estrutura obrigatória)

A ordem importa — abra pelo problema, não pela implementação:

1. Problema e restrições (não comece pela solução).
2. Uma execução curta + uma evidência de verificação.
3. A mesma política atravessando as duas IDEs.
4. Fechar com medição, limites e recomendação de uso.

---

## Pacote mínimo (checklist de entrega)

- [ ] README curto: problema, restrições, arquitetura, como executar.
- [ ] Ponto de entrada estável (`run.sh`).
- [ ] Estado e backlog persistidos (feature list + progresso).
- [ ] Smoke test e verificação com PASS/FAIL explícito.
- [ ] Handoff por feature: commit, resumo, testes, riscos.
- [ ] Trace da trajetória.
- [ ] Medição: custo, duração, erros, resultado.
- [ ] Recomendação: quando usar e quando não usar o harness.

## Critérios essenciais (autoavaliação)

- [ ] **Determinismo** — ordem, transições e teto de passos explícitos e testáveis.
- [ ] **Portabilidade** — mesmo fluxo em duas IDEs sem reescrever a política.
- [ ] **Verificação** — nenhuma feature concluída sem evidência e handoff.
- [ ] **Medição** — número real de custo/duração e leitura honesta dos limites.
- [ ] **Decisão** — README explica onde a solução se paga e onde a alternativa simples vence.

---

## Disciplina de sessão (Claude Code)

Mantenha o seu ritual: uma tarefa por sessão, revisar o plano antes de implementar, `/clear` entre sessões. O harness espelha isso — cada feature é uma sessão fechada com handoff. Se uma sessão começar a tocar duas features, pare e divida.
