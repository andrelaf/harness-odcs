# Decisão — onde este harness se paga, e onde ele perde

> Leitura de fechamento do projeto. Números vêm de `metrics/metrics.jsonl` e do
> código, não de estimativa.

---

## O que o harness entrega, em uma frase

Ele não classifica melhor. **Ele garante que nenhum campo atravessou o fluxo sem
decisão, que cada decisão tem justificativa e origem rastreável, e que nada
contraditório é persistido sem um humano dizer sim.**

Isso importa porque o classificador aqui é uma consulta a catálogo — 
`termo → entrada` — e uma pessoa faria o mesmo em minutos. O que uma pessoa não
faz de graça é repetir isso 200 vezes sem esquecer um campo, com o mesmo
vocabulário em semanas diferentes, deixando rastro que sobrevive à auditoria.

## O custo, medido

| | |
|---|---|
| Custo marginal por execução | ~6 s de máquina, ~11 invocações de container |
| Custo humano por execução, sem pendência | zero |
| Custo humano por execução, com pendência | uma leitura e um `approve` |
| Custo de token | **zero** — nenhum modelo roda no fluxo |
| Custo fixo de construção | 4 semanas, ~6.400 linhas de Rust |

99,6% do tempo de execução é espera de processo externo (530 ms por invocação de
`docker run`). O harness em si — máquina de estados, trace, estado, mapeamento e
classificação inteiros — custa os 0,4% restantes.

Duas consequências:

- **O custo escala com invocações, não com campos.** Classificar 9 campos ou 90
  custa praticamente o mesmo. Rodar o fluxo duas vezes custa o dobro.
- **Otimizar o harness não paga.** Qualquer ganho relevante está em reduzir
  partidas de container, não em código Rust.

## Onde se paga

Quando as três condições valem juntas:

1. **Volume.** Muitos contratos, ou poucos contratos revisados muitas vezes. O
   custo fixo é a construção; o marginal é desprezível.
2. **O rastro é requisito, não conforto.** Se alguém vai perguntar *"por que
   este campo foi classificado assim, e quem aprovou?"*, o `trace/` e o
   `evidence/` são o produto — não o contrato enriquecido.
3. **O vocabulário é compartilhado e tem dono.** É o glossário que faz `cpf`,
   `nr_cpf` e `documento_cpf` receberem a mesma resposta. Sem ele, a
   consistência entre contratos não existe e o harness só automatiza a
   inconsistência.

## Onde uma alternativa simples vence

**Um contrato único e trivial.** Revisar 9 campos na mão leva 15 minutos e não
exige Docker, Rust, glossário nem catálogo. Construir isto custou 4 semanas. Um
checklist no template do PR resolve, e resolve melhor.

**Domínio sem vocabulário estável.** Se cada campo exige julgamento próprio, o
harness devolve tudo como lacuna. Ele **converte ambiguidade em fila**, não a
resolve — e uma fila de 40 itens para decisão humana é pior que 40 decisões
tomadas direto, porque acrescenta a etapa de operar a máquina.

**Organização sem dono do glossário.** O acoplamento é deliberado: termo no
glossário sem entrada no catálogo é `FAIL`, não lacuna. Isso é uma virtude
quando alguém responde pelo vocabulário — obriga a manutenção. Quando ninguém
responde, o harness vira um bloqueio, e o time aprende a contorná-lo.

**Quando o que se quer é velocidade de classificação.** Um LLM classifica 200
campos em segundos. Perde exatamente o que este projeto foi construído para
garantir — cobertura verificável, justificativa por decisão e controle do que é
persistido —, mas se essas garantias não são requisito, ele é a escolha certa e
este harness é caro demais.

## Os limites do que foi construído

Ditos em voz alta, porque descobri-los na demo seria pior.

**Cobertura depende do glossário, não da ferramenta.** No primeiro contrato
real, **2 de 9 campos (22%) saíram como lacuna** — `segmento` e
`valor_total_compras`. O harness disse a verdade sobre não saber, que é o
comportamento correto. Mas 78% de classificação automática é o teto do
vocabulário atual, não uma limitação a ser "melhorada no código".

**Um contrato, sintético, plano.** Propriedades aninhadas do ODCS não foram
exercitadas: a extração lê as propriedades de topo do JSON Schema exportado. O
primeiro contrato com objeto aninhado vai expor isso.

**A reescrita do contrato perde comentários.** O YAML é reserializado inteiro.
Ordem de chaves e propriedades é preservada; comentários e linhas em branco,
não.

**A escala esbarra na partida do container.** 530 ms por invocação, ~11 por
execução. Mil contratos são cerca de 1h40 só de partida de Docker. O desenho
precisaria de lote ou de um CLI de vida longa — e essa é a mudança que o número
justifica, nenhuma outra.

**Nenhum modelo decide nada.** Foi a decisão de projeto, e é de onde vêm as
garantias. Mas significa que o harness não acrescenta inteligência de
classificação: a inteligência está no glossário e no catálogo, mantidos por
pessoas. Quem esperar que ele "descubra" que `valor_total_compras` é dado
pessoal vai se decepcionar — e deve, porque adivinhar isso sozinho é
exatamente o que ele não pode fazer.

**O domínio ficou maior que o harness.** Fora testes: ~2.675 linhas em
`features/` contra ~2.421 no núcleo. O brief avisava que o risco nº 1 era o
classificador engolir o projeto. Não engoliu no sentido que importa — a regra de
classificação continua sendo uma consulta a catálogo, sem heurística — mas F4
precisou escrever ODCS de volta, e escrever é caro. Se houvesse uma quinta
feature, eu cortaria escopo antes de escrever a primeira linha.

**A verificação externa depende de exit code e de arquivo, não de parser.**
`--json` existe em `status`, `doctor` e `metrics`, e é recusado nos comandos que
mutam estado. Quem integra o harness em CI lê o exit code — `0`, `1`, `3`, `5` —
e, se precisar do conteúdo, os arquivos em `state/` e `metrics/` já são JSON.
A flag é conveniência de pipe, não capacidade nova, e vale saber disso antes de
construir automação em cima dela.

## O que eu faria diferente

- **Branch por feature virou pilha linear.** Cada branch nasceu do topo da
  anterior em vez da `main`, e a `main` nunca recebeu nada. Funciona, mas o
  histórico não mostra quatro entregas independentes — mostra uma só, longa.
  Merge de volta ao fim de cada feature, desde F1.
- **`evidence/` cresce sem política de retenção.** 14 execuções já deixaram
  dezenas de arquivos. Precisa de expiração antes de virar problema.
- **O hash do gate cobre o texto do item.** Reescrever a frase de um `detalhe`
  invalida aprovações antigas. Falha fechada, que é o lado certo de errar, mas
  incomoda no uso.

## Recomendação

**Use** quando houver contratos em volume, exigência de rastreabilidade e um
dono para o vocabulário. O valor está no `trace/`, no `evidence/` e no gate —
não na velocidade.

**Não use** para um contrato único, para um domínio sem vocabulário estável, ou
quando ninguém for responder pelo glossário. Nesses casos, revisão humana com
checklist é mais rápida, mais barata e igualmente confiável.

A pergunta que separa os dois casos não é *"quantos contratos você tem?"* — é
**"alguém vai ter que provar, depois, por que este campo foi classificado
assim?"**. Se a resposta for não, este harness é caro demais.
