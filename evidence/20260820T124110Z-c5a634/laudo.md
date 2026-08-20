# Laudo de classificacao de privacidade

**Contrato** `contracts/pedidos/contract.odcs.yaml` v1.0.0  
**sha256** `43fcfea1594b524c928b4cf6e3b10bdb35ec15eae3c4a2aebe6158a7e60e23cf`

O sha256 acima e o do contrato **classificado** — o arquivo que esta neste repositorio, ao lado deste laudo. Quem auditar confere a correspondencia com qualquer ferramenta de hash, sem depender deste projeto.

## Criterio aplicado

| Insumo | Versao | sha256 |
|---|---|---|
| Glossario `glossary/glossario.yaml` | 1.1.0 | `3826d52d24ded406` |
| Catalogo `classification/catalogo-lgpd.yaml` | 1.1.0 | `f7245b3fbdcb5a55` |

A classificacao e **consulta a catalogo**: cada campo do contrato e mapeado a um termo do glossario, e o termo carrega a classificacao, a justificativa e a base legal. Nao ha inferencia sobre nome de campo e nenhum modelo participa da decisao — dois campos que casam com o mesmo termo recebem a mesma resposta em qualquer contrato, em qualquer data.

## Resumo

- 8 campo(s) analisado(s)
- 8 classificado(s) — 5 PII, 0 sensivel
- por nivel: 4 confidential, 3 internal, 1 restricted
- 0 sem classificacao

## Classificacao por campo

| Campo | Termo | `classification` | PII | Sensivel | Base legal | Justificativa |
|---|---|---|---|---|---|---|
| `_id` | `registro.identificador_tecnico` | `internal` | nao | nao | LGPD art. 5, I — por exclusao | Chave primaria atribuida pelo banco. Identifica o registro dentro da colecao e nao carrega significado fora dela — diferente do identificador interno de pessoa, que existe para apontar para alguem. |
| `cliente.cpf` | `pessoa.cpf` | `restricted` | sim | nao | LGPD art. 5, I | Identificador univoco de pessoa natural emitido pela Receita Federal. Identifica sozinho e serve de chave de cruzamento com praticamente qualquer outra base do pais. |
| `cliente.email` | `contato.email` | `confidential` | sim | nao | LGPD art. 5, I | Endereco de contato individual. Na pratica funciona como identificador de conta e alcanca a pessoa diretamente. |
| `cliente.nome_completo` | `pessoa.nome_completo` | `confidential` | sim | nao | LGPD art. 5, I | Nome civil identifica a pessoa natural sem necessidade de cruzamento com outra base. |
| `data_pedido` | `pedido.data` | `internal` | nao | nao | LGPD art. 5, I — por exclusao | Descreve quando a transacao ocorreu, nao quem a fez. |
| `entregas[].cep` | `endereco.cep` | `confidential` | sim | nao | LGPD art. 5, I | CEP de residencia nao identifica sozinho, mas no Brasil chega ao nivel de logradouro e reidentifica quando combinado com data de nascimento. |
| `entregas[].data_nascimento_recebedor` | `pessoa.data_nascimento` | `confidential` | sim | nao | LGPD art. 5, I | Nao identifica sozinha, mas compoe o quase-identificador classico (data de nascimento + sexo + localidade), que reidentifica parcela alta de uma populacao quando combinado. |
| `valor_total` | `pedido.valor_total` | `internal` | nao | nao | LGPD art. 5, I — por exclusao | Valor de uma transacao isolada descreve o pedido. O acumulado por pessoa e outro termo, e classificado como dado pessoal. |

## Pendencias de decisao humana

Nenhuma. Todo campo classificado veio do catalogo sem contrariar decisao anterior, e nenhum campo ficou sem decisao.

## Sobre este documento

E deterministico: o mesmo contrato, com o mesmo glossario e o mesmo catalogo, produz este arquivo byte a byte. Nao ha data no corpo de proposito — a data de emissao e a do commit, e o Git ja responde por ela.

Nao ha aprovacao no corpo pelo mesmo motivo: este e o laudo tecnico, e quem assina e o merge. A revisao que o autorizou fica no historico do repositorio, presa ao mesmo sha256 do cabecalho.

Subir a versao do catalogo pode mudar a classificacao de um termo sem que o contrato mude. Quando isso acontecer, este laudo continua valendo para o criterio que o gerou, e um novo sera emitido ao lado dele.
