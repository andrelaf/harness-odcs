# Laudo de classificacao de privacidade

**Contrato** `contracts/clientes/contract.odcs.yaml` v1.0.0  
**sha256** `e6b701d7f4b2bd6b27925c2f766785db7551468280a888923f586d02bfed3604`

O sha256 acima e o do contrato **classificado** — o arquivo que esta neste repositorio, ao lado deste laudo. Quem auditar confere a correspondencia com qualquer ferramenta de hash, sem depender deste projeto.

## Criterio aplicado

| Insumo | Versao | sha256 |
|---|---|---|
| Glossario `glossary/glossario.yaml` | 1.1.0 | `3826d52d24ded406` |
| Catalogo `classification/catalogo-lgpd.yaml` | 1.1.0 | `f7245b3fbdcb5a55` |

A classificacao e **consulta a catalogo**: cada campo do contrato e mapeado a um termo do glossario, e o termo carrega a classificacao, a justificativa e a base legal. Nao ha inferencia sobre nome de campo e nenhum modelo participa da decisao — dois campos que casam com o mesmo termo recebem a mesma resposta em qualquer contrato, em qualquer data.

## Resumo

- 9 campo(s) analisado(s)
- 9 classificado(s) — 8 PII, 0 sensivel
- por nivel: 7 confidential, 1 internal, 1 restricted
- 0 sem classificacao

## Classificacao por campo

| Campo | Termo | `classification` | PII | Sensivel | Base legal | Justificativa |
|---|---|---|---|---|---|---|
| `cep` | `endereco.cep` | `confidential` | sim | nao | LGPD art. 5, I | CEP de residencia nao identifica sozinho, mas no Brasil chega ao nivel de logradouro e reidentifica quando combinado com data de nascimento. |
| `cpf` | `pessoa.cpf` | `restricted` | sim | nao | LGPD art. 5, I | Identificador univoco de pessoa natural emitido pela Receita Federal. Identifica sozinho e serve de chave de cruzamento com praticamente qualquer outra base do pais. |
| `data_cadastro` | `cadastro.data_criacao` | `internal` | nao | nao | LGPD art. 5, I — por exclusao | Descreve o ciclo de vida do registro, nao a pessoa: isolada, nao identifica ninguem nem revela atributo de pessoa natural. Numa linha ja identificada ela herda o carater pessoal do conjunto — risco que se trata no nivel do dataset, nao no do campo. |
| `data_nascimento` | `pessoa.data_nascimento` | `confidential` | sim | nao | LGPD art. 5, I | Nao identifica sozinha, mas compoe o quase-identificador classico (data de nascimento + sexo + localidade), que reidentifica parcela alta de uma populacao quando combinado. |
| `email` | `contato.email` | `confidential` | sim | nao | LGPD art. 5, I | Endereco de contato individual. Na pratica funciona como identificador de conta e alcanca a pessoa diretamente. |
| `id_cliente` | `pessoa.identificador_interno` | `confidential` | sim | nao | LGPD art. 5, I c/c art. 13, par. 4 | Chave pseudonimizada. A pseudonimizacao nao descaracteriza o dado pessoal enquanto a organizacao mantiver, ainda que separada, a informacao adicional que permite reassociar a chave a pessoa. |
| `nome_completo` | `pessoa.nome_completo` | `confidential` | sim | nao | LGPD art. 5, I | Nome civil identifica a pessoa natural sem necessidade de cruzamento com outra base. |
| `segmento` | `perfil.segmento` | `confidential` | sim | nao | LGPD art. 5, I | Nao identifica sozinho e nao veio da pessoa: foi atribuido a ela. Ainda assim e dado pessoal, porque qualifica a pessoa identificada na linha e alimenta decisao comercial sobre ela. |
| `valor_total_compras` | `perfil.valor_total_compras` | `confidential` | sim | nao | LGPD art. 5, I | Agregado de comportamento de consumo vinculado a pessoa. Nao identifica sozinho, mas descreve a pessoa — diferente do valor de um pedido, que descreve a transacao. |

## Pendencias de decisao humana

Nenhuma. Todo campo classificado veio do catalogo sem contrariar decisao anterior, e nenhum campo ficou sem decisao.

## Sobre este documento

E deterministico: o mesmo contrato, com o mesmo glossario e o mesmo catalogo, produz este arquivo byte a byte. Nao ha data no corpo de proposito — a data de emissao e a do commit, e o Git ja responde por ela.

Nao ha aprovacao no corpo pelo mesmo motivo: este e o laudo tecnico, e quem assina e o merge. A revisao que o autorizou fica no historico do repositorio, presa ao mesmo sha256 do cabecalho.

Subir a versao do catalogo pode mudar a classificacao de um termo sem que o contrato mude. Quando isso acontecer, este laudo continua valendo para o criterio que o gerou, e um novo sera emitido ao lado dele.
