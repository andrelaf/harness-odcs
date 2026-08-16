# Cobertura — o que o harness enxerga, e o que passa por baixo

Um experimento com um contrato de coleção MongoDB, feito para responder uma
pergunta de governança: **este processo cobre o que promete?**

A resposta curta é **não, ainda não** — e o modo como ele falha importa mais que
o fato de falhar.

---

## O experimento

Um contrato ODCS descrevendo uma coleção de pedidos, no formato que se usa em
MongoDB: documento do cliente **embutido** no pedido para evitar join, e um array
de tentativas de entrega.

```yaml
properties:
  - name: _id            # objectId
  - name: data_pedido
  - name: valor_total
  - name: cliente        # objeto aninhado
    properties:
      - name: cpf
      - name: nome_completo
      - name: email
  - name: entregas       # array de objetos
    items:
      properties:
        - name: cep
        - name: data_nascimento_recebedor
```

Dez campos reais. **Cinco deles são dado pessoal**: CPF, nome completo, e-mail,
CEP e data de nascimento.

O contrato está em [`contracts/pedidos/`](../contracts/pedidos/), preservado como
evidência do experimento.

## O resultado

```
$ ./run.sh check --contrato contracts/pedidos/contract.odcs.yaml
veredito   FAIL
campos     5 — 0 classificado(s), 5 lacuna(s), 0 conflito(s)
  gate     5 item(ns)
             [lacuna] _id
             [lacuna] cliente
             [lacuna] data_pedido
             [lacuna] entregas
             [lacuna] valor_total
```

**Cinco campos vistos, e nenhum deles é dado pessoal.** O harness viu `_id`,
`data_pedido`, `valor_total` e os dois **nomes de container** — `cliente` e
`entregas`. Os cinco campos de PII estão exatamente entre os invisíveis.

## Onde exatamente está a falha

Não é no `datacontract-cli`. O JSON Schema que ele exporta **carrega a árvore
inteira**:

```
_id          [string]
data_pedido  [string]
valor_total  [number]
cliente      [object]
  cpf            [string]
  nome_completo  [string]
  email          [string, null]
entregas     [array, null]
  items → properties → cep, data_nascimento_recebedor
```

A informação está toda lá. A perda acontece na extração do harness, em
[`src/features/contrato.rs`](../src/features/contrato.rs):

```rust
Ok(schema
    .properties
    .into_iter()          // ← um nível, e só
    .map(|(nome, prop)| Campo { … })
    .collect())
```

Um `into_iter()` plano sobre o primeiro nível. Subárvores são descartadas em
silêncio.

A ironia é que o cabeçalho desse mesmo arquivo já previa o problema:

> *"ler `schema[].properties[].name` do YAML parece trivial até o primeiro
> contrato com propriedade aninhada"*

O harness evitou parsear o YAML — decisão correta — e depois cometeu o mesmo erro
um passo adiante, sobre o JSON Schema.

E `decisao.md` já tinha registrado a dívida, quatro semanas antes:

> *"Propriedades aninhadas do ODCS não foram exercitadas… O primeiro contrato
> com objeto aninhado vai expor isso."*

Expôs.

---

## A leitura de governança

### O que salvou

**O harness falha fechado.** `cliente` e `entregas` voltaram como **lacunas**, não
como "sem PII". Ninguém recebeu um laudo dizendo que o pedido não tem dado
pessoal: recebeu um laudo dizendo *"não sei o que são estes cinco campos"*, e o
veredito bloqueia esperando decisão humana.

Isso é a diferença entre um sistema que erra e um que mente. O gate cumpriu seu
papel.

### O que não salvou, e é grave

**O laudo entende menos do que aparenta.** Ele afirma sobre `cliente` como se
fosse um campo, quando são três — e não menciona em lugar nenhum que existe um
CPF ali dentro. Quem ler o laudo procurando "este contrato tem CPF?" **não
encontra**.

**E o pior caminho está aberto.** Basta alguém adicionar `cliente` ao glossário —
digamos, como *"dados do cliente"*, classificação `internal` — e a lacuna some.
O harness passa a `PASS`, escreve `classification: internal` no campo `cliente`,
e emite um laudo afirmando isso com base legal e justificativa. **O CPF fica
coberto por uma classificação errada, com documento assinando embaixo.**

Não é hipotético: é o caminho de menor esforço para quem quer fechar a lacuna
rápido. A ferramenta que existe para impedir classificação displicente teria
produzido a prova documental dela.

### O veredito

Para contratos **planos** — tabelas relacionais, CSV, Parquet com schema raso —
a cobertura é real e o processo entrega o que promete.

Para contratos com **estrutura aninhada** — MongoDB, JSON de API, Avro com
records — a cobertura é **parcial e enganosa**. Não use este processo como prova
de conformidade sobre eles até a extração ser recursiva.

---

## O que a correção exige

Não é grande, mas toca três lugares:

**1. Extração recursiva** (`contrato.rs`). Descer em `properties` de objetos e em
`items.properties` de arrays, produzindo caminhos pontilhados: `cliente.cpf`,
`entregas[].cep`. O JSON Schema já entrega tudo — é percorrer a árvore.

**2. Glossário por caminho** (`f2_mapear.rs`). Hoje o casamento é por nome
simples. Com `cliente.cpf`, a busca precisa considerar o último segmento (`cpf`
casa com `pessoa.cpf`) sem perder o caminho completo no laudo — senão dois `cpf`
em ramos diferentes viram um só.

**3. Enriquecimento aninhado** (`f4_gate.rs`). Escrever `classification` na
propriedade certa, dentro da árvore. É a parte mais cara: hoje a reescrita opera
sobre a lista plana de propriedades de topo.

Estimativa honesta: o item 1 é uma tarde; os itens 2 e 3, juntos, são o mesmo
tamanho de uma das features originais (F2 ou F3) — dias, não horas. E `decisao.md`
já avisava que **escrever ODCS de volta é a parte cara**.

Enquanto não for feito, a mitigação que funciona é de processo, não de código:
**tratar contrato com objeto aninhado como fora do escopo automatizado**, e
revisar na mão. O harness não sabe dizer o que não viu — e um contrato que ele
não cobre inteiro não deveria receber laudo, porque o laudo passa a valer como
prova de algo que ninguém verificou.
