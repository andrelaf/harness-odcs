# Cobertura — o que o harness enxerga, e o que passa por baixo

Um experimento com um contrato de coleção MongoDB, feito para responder uma
pergunta de governança: **este processo cobre o que promete?**

A resposta era **não**. Desde a F5 é **sim**, e este documento guarda os dois
estados: o que falhava, por quê, e o que a correção mudou. A falha vale mais
registrada que apagada — ela explica por que o desenho é como é.

> **Resolvido em F5** — [`spec-f5-aninhado.md`](spec-f5-aninhado.md). A extração
> passou a percorrer a árvore, e `contracts/pedidos/` saiu de 5 nós vistos para
> **8 campos**, com `cliente.cpf` classificado como `restricted`.

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

Oito campos de dado — mais dois containers, `cliente` e `entregas`, que agrupam
sem carregar valor. **Cinco dos oito são dado pessoal**: CPF, nome completo,
e-mail, CEP e data de nascimento.

O contrato está em [`contracts/pedidos/`](../contracts/pedidos/), preservado como
evidência do experimento.

## O resultado — antes da F5

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
[`crates/laudo/src/features/contrato.rs`](../crates/laudo/src/features/contrato.rs):

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

### O veredito daquele momento

Para contratos **planos**, a cobertura era real. Para contratos com **estrutura
aninhada**, era parcial e enganosa — e a recomendação registrada foi não usar o
processo como prova de conformidade sobre eles enquanto a extração não descesse
na árvore.

É essa recomendação que a F5 revoga.

---

## O resultado — depois da F5

```
$ ./run.sh check --contrato contracts/pedidos/contract.odcs.yaml
veredito   BLOQUEADO
campos     8 — 4 classificado(s), 4 lacuna(s), 0 conflito(s)
  gate     [lacuna] _id · data_pedido · valor_total
           [lacuna] entregas[].data_nascimento_recebedor
```

Os campos aparecem com o caminho até eles, e o `classification` é escrito **na
folha** — `cliente` continua sem classificação nenhuma, que é o correto: ele é
agrupamento, não dado.

A quinta lacuna, `entregas[].data_nascimento_recebedor`, **não** é falha da
ferramenta: o glossário cobre `data_nascimento` da pessoa titular, e a data de
nascimento de quem recebeu a entrega é outro dado, que ninguém cadastrou. O
harness diz que não sabe — comportamento certo. Fechá-la é decisão de quem
responde pelo vocabulário.

## O que a correção exigiu

Não foi grande, mas tocou três lugares:

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

A estimativa era "uma tarde para o item 1, dias para os outros dois". Saiu mais
barato que isso, e o motivo é instrutivo: as três funções já eram puras e
testadas, então a mudança foi **trocar iteração por recursão** em três lugares
bem delimitados, sem tocar em regra de classificação nenhuma.

O que protegeu contra o risco silencioso — escrever no nó errado — foi a
assertiva estar no `verify` da feature, e não num teste opcional: um campo cujo
nome seja prefixo de outro reprova o run, porque isso significaria container
reportado como folha.

### O que continua fora

`$ref` e composição (`allOf`, `oneOf`) não são percorridos. O motor não os produz
para os contratos que este projeto exercita; aparecendo, o nó vira lacuna — que é
o comportamento certo para o que não se entende.
