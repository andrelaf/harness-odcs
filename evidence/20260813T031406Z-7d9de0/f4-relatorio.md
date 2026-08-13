# F4 — Enriquecimento, lacunas e gate

- Contrato: `contracts/clientes/contract.odcs.yaml` (sha256 `19121d7f9d6672a7` na entrada)
- Glossario v1.0.0 · catalogo v1.0.0
- 9 campo(s) — 7 classificado(s): 7 primeira classificacao, 0 inalterado(s), 0 reclassificacao(oes)
- 2 lacuna(s), 0 conflito(s)

## Pendencias de decisao humana

Submetidas ao gate e **liberadas por decisao humana em 2026-08-13T03:14:02.284265Z** — pedido `4f773bab5f6e400f`.

| Tipo | Campo | Detalhe |
|---|---|---|
| `lacuna` | `segmento` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |
| `lacuna` | `valor_total_compras` | campo sem termo no glossario — segue sem classificacao no contrato ate decisao humana |

## Classificacao aplicada

| Campo | Termo | Mudanca | Proposto | Declarado antes |
|---|---|---|---|---|
| `cep` | `endereco.cep` | primeira classificacao | confidential, pii | nada declarado |
| `cpf` | `pessoa.cpf` | primeira classificacao | restricted, pii | nada declarado |
| `data_cadastro` | `cadastro.data_criacao` | primeira classificacao | internal, nao pii | nada declarado |
| `data_nascimento` | `pessoa.data_nascimento` | primeira classificacao | confidential, pii | nada declarado |
| `email` | `contato.email` | primeira classificacao | confidential, pii | nada declarado |
| `id_cliente` | `pessoa.identificador_interno` | primeira classificacao | confidential, pii | nada declarado |
| `nome_completo` | `pessoa.nome_completo` | primeira classificacao | confidential, pii | nada declarado |
| `segmento` | — | **lacuna** | nada declarado | nada declarado |
| `valor_total_compras` | — | **lacuna** | nada declarado | nada declarado |

Campo sem classificacao nao e tocado no contrato: o harness nao escreve "nao sei" num contrato de dados. Resolver a lacuna e ampliar o glossario e o catalogo — ou aceitar, por decisao registrada, que o campo siga sem classificacao.

O que foi escrito em cada propriedade classificada e ODCS 3.1.0: `classification`, `tags: [pii, sensitive]` e `authoritativeDefinitions` apontando para o termo do glossario.
