# Spec F6 — A divergência sobrevive

> Feature `f6-divergencia`. Corrige um comportamento em que o harness resolvia
> sozinho uma contradição que ele não tem autoridade para resolver.

---

## O defeito

O contrato declara `classification: internal` num campo; o catálogo diz
`restricted`. O `check` detectava e reportava:

```
[reclassificacao] cliente.cpf — o contrato declara `internal, pii`
                                e o catalogo diz `restricted, pii`
```

E então o `aplicar` **sobrescrevia o campo para `restricted`**. O `git diff`
ficava vazio: a declaração humana desaparecia sem rastro, e o laudo afirmava
`restricted` sem mencionar que alguém tinha discordado.

O `docs/decisao.md` abre com a promessa que isso viola:

> *"nada contraditório é persistido sem um humano dizer sim"*

O item de gate existia mas não tinha consequência. No fluxo local (`next`) ele
bloqueia até `approve`; no fluxo de pull request, decorava o relatório.

## A distinção que faltava

Os dois tipos de gate não são equivalentes:

| Tipo | O que `aplicar` faz no campo | Aceitável? |
|---|---|---|
| **lacuna** | nada — segue sem classificação | **sim**, escrever é inofensivo, e o laudo precisa ser emitido para o revisor |
| **reclassificação** | **sobrescreve** a declaração humana | **não**, destrói a divergência |

"Gate aberto não impede aplicar" estava certo para o primeiro caso e errado para
o segundo.

## A regra

**Campo em reclassificação não é escrito.** O contrato preserva o que a pessoa
declarou; o gate nomeia a divergência; o laudo registra as duas versões.

Os demais campos são enriquecidos normalmente — preservar um não respinga nos
outros.

`Conflito` (contrato declara algo, catálogo não tem o termo) já era seguro por
construção: sem classificação a propor, o campo nunca entrava na escrita.

## O que muda no veredito

Antes, uma divergência produzia **duas** reprovações ao mesmo tempo: o gate
aberto e um `[aplicacao] o contrato não está com a classificação aplicada` —
porque o proposto diferia do disco para sempre.

Agora o proposto e o disco coincidem, e sobra só o gate: veredito `5`,
**bloqueado aguardando decisão humana**. Divergência não é erro de aplicação, é
decisão pendente — e o exit code passa a dizer isso.

## Como a divergência se fecha

Três saídas, todas deliberadas e registradas:

1. **O catálogo vale** — corrige o contrato, a divergência some, veredito `0`
2. **O contrato vale** — corrige o catálogo, que tem outro dono, e sobe o pin
3. **Fica como está, por ora** — aprova com a divergência documentada no laudo

A terceira é a que o defeito tornava impossível: uma **divergência aprovada**,
com nome e data no histórico. É resultado legítimo de governança; o que não pode
é a divergência sumir sem ninguém ter dito nada.

## Verificação

| Asserção | Por quê |
|---|---|
| campo preservado mantém o que o contrato declarava | é o ponto da feature |
| preservar um campo não impede os outros de serem escritos | evita regressão por excesso de zelo |
| contrato sem divergência produz o mesmo laudo byte a byte | esta feature não podia mudar o caso comum |
| veredito de contrato divergente é `5`, não `1` | decisão pendente ≠ erro |

## Fora de escopo

**Distinguir despromoção de promoção.** Rebaixar um campo PII é o caminho por
onde dado sensível vaza; subir é conservador. Hoje os dois param no gate, e
enquanto o volume for baixo essa é a falha para o lado certo. Se incomodar,
tratar despromoção como gate e promoção como aviso é a saída — e é decisão de
quem responde pelo vocabulário, não de código.

**Marcar a linha divergente na tabela principal do laudo.** Hoje a tabela por
campo mostra o que o catálogo diz, e a divergência aparece na tabela de
pendências. Os dois fatos estão no documento; juntá-los numa linha só seria
melhor de ler, e é cosmético.
