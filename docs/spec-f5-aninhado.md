# Spec F5 — Cobertura de estrutura aninhada

> Feature `f5-aninhado`. Fecha o limite medido em [`cobertura.md`](cobertura.md):
> o harness enxerga apenas o primeiro nível do contrato, e num documento MongoDB
> os campos de dado pessoal ficam invisíveis.

---

## O problema, em uma linha

`contracts/pedidos/` declara oito campos de dado, cinco deles pessoais. O harness
vê **cinco nós** — três campos de topo e dois nomes de container — e nenhum dos
cinco é dado pessoal.

A informação existe: o JSON Schema exportado pelo motor carrega a árvore
inteira. A perda está na extração, que itera um nível só.

## O que muda

**Só a leitura da árvore.** Nenhuma regra de classificação, nenhum formato de
laudo, nenhum exit code. Um campo que hoje é classificado continua sendo, com o
mesmo termo e a mesma justificativa.

### 1. Caminho em vez de nome

O identificador de um campo passa a ser o **caminho** até ele:

```
_id                                  campo de topo, como hoje
cliente.cpf                          dentro de objeto
entregas[].cep                       dentro de array de objetos
```

`[]` marca travessia de array. É notação de leitura, não de índice: o contrato
descreve a forma de todo elemento, não de um elemento específico.

### 2. Container não é campo

Um objeto ou array que **tem filhos** deixa de ser reportado como campo. Ele não
carrega valor para classificar; é agrupamento.

Consequência direta: `cliente` e `entregas` somem da lista, e no lugar deles
entram os cinco campos que estavam escondidos. `contracts/pedidos/` sai de 5 nós
para **8 campos**.

Objeto declarado sem filhos continua sendo campo — sem estrutura, ele é um valor.

### 3. Casamento com o glossário: caminho, depois folha

Para `cliente.cpf`, a busca tenta nesta ordem:

1. o caminho inteiro — `cliente.cpf`
2. o último segmento — `cpf`

O primeiro permite ao glossário desambiguar quando o mesmo nome significa coisas
diferentes em ramos diferentes. O segundo é o que faz `cliente.cpf` casar com
`pessoa.cpf` sem ninguém precisar cadastrar caminho nenhum.

**O laudo registra sempre o caminho completo.** Dois `cpf` em ramos distintos são
dois campos, e um laudo que os chame de `cpf` esconde justamente o que a auditoria
precisa distinguir.

### 4. Escrita no lugar certo

O enriquecimento passa a navegar a árvore do ODCS para escrever `classification`
na propriedade correta — descendo por `properties` de objetos e por
`items.properties` de arrays.

É a parte cara, e é onde a feature pode falhar de formas silenciosas: escrever no
nó errado produz um contrato que passa no lint e mente.

## Verificação

`verify` reprova se qualquer uma destas não valer para `contracts/pedidos/`:

| Asserção | Por quê |
|---|---|
| 8 campos extraídos | prova que a árvore foi percorrida |
| nenhum campo com filhos na lista | container não é campo |
| `cliente.cpf` classificado como `restricted` | o caminho casou com `pessoa.cpf` |
| `entregas[].data_nascimento_recebedor` classificado | array foi percorrido |
| o YAML enriquecido tem `classification` **dentro** de `cliente` | escreveu no nó certo |
| lint do enriquecido passa | não corrompeu a estrutura |

O contrato plano de `contracts/clientes/` tem de continuar produzindo **o mesmo
laudo byte a byte**. Se mudar, a feature alterou comportamento de contrato plano
— e não era para.

## Fora de escopo

- **Glossário com termos por caminho.** O casamento aceita caminho, mas nenhum
  termo novo é cadastrado. Ampliar vocabulário é decisão de quem responde por ele.
- **Arrays de escalares.** `tags: [string]` continua sendo um campo só. Não há o
  que descer.
- **`$ref` e composição (`allOf`, `oneOf`).** O motor não os produz para os
  contratos que este projeto exercita. Aparecendo, viram lacuna — que é o
  comportamento certo para o que não se entende.

## Risco

**O maior é escrever no nó errado em silêncio.** Um `classification` no
container em vez de na folha passa no lint, produz laudo e afirma o que ninguém
verificou — exatamente o cenário que `cobertura.md` descreve como o pior caminho.

Mitigação: a asserção de que o `classification` aparece **dentro** de `cliente`, e
não ao lado dele, é parte do `verify` e não de um teste opcional.
