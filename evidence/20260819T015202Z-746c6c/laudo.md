# Laudo de classificacao de privacidade

**Contrato** `contracts/pedidos/contract.odcs.yaml` v1.0.0  
**sha256** `19e01791626c92112071cfa30a36ab5399853f51bb30f43bec8732e7482d7a6d`

O sha256 acima e o do contrato **classificado** — o arquivo que esta neste repositorio, ao lado deste laudo. Quem auditar confere a correspondencia com qualquer ferramenta de hash, sem depender deste projeto.

## Criterio aplicado

| Insumo | Versao | sha256 |
|---|---|---|
| Glossario `glossary/glossario.yaml` | 1.0.0 | `d4f3c8d60452aaaa` |
| Catalogo `classification/catalogo-lgpd.yaml` | 1.0.0 | `af8c1227a6af10e9` |

A classificacao e **consulta a catalogo**: cada campo do contrato e mapeado a um termo do glossario, e o termo carrega a classificacao, a justificativa e a base legal. Nao ha inferencia sobre nome de campo e nenhum modelo participa da decisao — dois campos que casam com o mesmo termo recebem a mesma resposta em qualquer contrato, em qualquer data.

## Resumo

- 8 campo(s) analisado(s)
- 4 classificado(s) — 4 PII, 0 sensivel
- por nivel: 3 confidential, 1 restricted
- 4 sem classificacao

## Classificacao por campo

| Campo | Termo | `classification` | PII | Sensivel | Base legal | Justificativa |
|---|---|---|---|---|---|---|
| `_id` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |
| `cliente.cpf` | `pessoa.cpf` | `restricted` | sim | nao | LGPD art. 5, I | Identificador univoco de pessoa natural emitido pela Receita Federal. Identifica sozinho e serve de chave de cruzamento com praticamente qualquer outra base do pais. |
| `cliente.email` | `contato.email` | `confidential` | sim | nao | LGPD art. 5, I | Endereco de contato individual. Na pratica funciona como identificador de conta e alcanca a pessoa diretamente. |
| `cliente.nome_completo` | `pessoa.nome_completo` | `confidential` | sim | nao | LGPD art. 5, I | Nome civil identifica a pessoa natural sem necessidade de cruzamento com outra base. |
| `data_pedido` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |
| `entregas[].cep` | `endereco.cep` | `confidential` | sim | nao | LGPD art. 5, I | CEP de residencia nao identifica sozinho, mas no Brasil chega ao nivel de logradouro e reidentifica quando combinado com data de nascimento. |
| `entregas[].data_nascimento_recebedor` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |
| `valor_total` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |

## Pendencias de decisao humana

4 item(ns), sob o pedido `a6a710bfc83499b6`. Enquanto houver pendencia aqui, o campo correspondente **nao** recebe classificacao no contrato: o harness nao escreve "nao sei" num contrato de dados.

| Tipo | Campo | Detalhe |
|---|---|---|
| `lacuna` | `_id` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |
| `lacuna` | `data_pedido` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |
| `lacuna` | `entregas[].data_nascimento_recebedor` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |
| `lacuna` | `valor_total` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |

Lacuna se resolve ampliando o glossario e o catalogo — ou aceitando, por decisao registrada, que o campo siga sem classificacao.

## Sobre este documento

E deterministico: o mesmo contrato, com o mesmo glossario e o mesmo catalogo, produz este arquivo byte a byte. Nao ha data no corpo de proposito — a data de emissao e a do commit, e o Git ja responde por ela.

Nao ha aprovacao no corpo pelo mesmo motivo: este e o laudo tecnico, e quem assina e o merge. A revisao que o autorizou fica no historico do repositorio, presa ao mesmo sha256 do cabecalho.

Subir a versao do catalogo pode mudar a classificacao de um termo sem que o contrato mude. Quando isso acontecer, este laudo continua valendo para o criterio que o gerou, e um novo sera emitido ao lado dele.
