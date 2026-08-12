# F2 — Mapeamento campo -> glossario

- Contrato: `contracts/clientes/contract.odcs.yaml` (sha256 `19121d7f9d6672a7`)
- Glossario: `glossary/glossario.yaml` v1.0.0 (sha256 `d4f3c8d60452aaaa`)
- Cobertura: 9 campo(s) — 7 mapeado(s), 2 lacuna(s)

| Campo | Tipo | Decisao | Termo | Justificativa |
|---|---|---|---|---|
| `cep` | string\|null | mapeado | `endereco.cep` | `cep` normalizado e `cep`, que o termo `endereco.cep` declara |
| `cpf` | string | mapeado | `pessoa.cpf` | `cpf` normalizado e `cpf`, que o termo `pessoa.cpf` declara |
| `data_cadastro` | string\|null | mapeado | `cadastro.data_criacao` | `data_cadastro` normalizado e `data_cadastro`, que o termo `cadastro.data_criacao` declara |
| `data_nascimento` | string\|null | mapeado | `pessoa.data_nascimento` | `data_nascimento` normalizado e `data_nascimento`, que o termo `pessoa.data_nascimento` declara |
| `email` | string | mapeado | `contato.email` | `email` normalizado e `email`, que o termo `contato.email` declara |
| `id_cliente` | string | mapeado | `pessoa.identificador_interno` | `id_cliente` normalizado e `id_cliente`, que o termo `pessoa.identificador_interno` declara |
| `nome_completo` | string | mapeado | `pessoa.nome_completo` | `nome_completo` normalizado e `nome_completo`, que o termo `pessoa.nome_completo` declara |
| `segmento` | string\|null | **lacuna** | — | nenhum termo do glossario declara a chave `segmento` — lacuna para decisao humana |
| `valor_total_compras` | number\|null | **lacuna** | — | nenhum termo do glossario declara a chave `valor_total_compras` — lacuna para decisao humana |

Lacuna nao reprova F2: a cobertura exigida e de decisao, nao de acerto. O relatorio de lacunas para decisao humana e F4.
