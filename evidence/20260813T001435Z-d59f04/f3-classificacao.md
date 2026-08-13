# F3 — Classificacao de privacidade (LGPD)

- Contrato: `contracts/clientes/contract.odcs.yaml` (sha256 `19121d7f9d6672a7`)
- Glossario: `glossary/glossario.yaml` v1.0.0 (sha256 `d4f3c8d60452aaaa`)
- Catalogo: `classification/catalogo-lgpd.yaml` v1.0.0 (sha256 `af8c1227a6af10e9`)
- Cobertura: 9 campo(s) — 7 classificado(s), 2 sem classificacao
- 6 campo(s) PII, 0 sensivel; por nivel: 5 confidential, 1 internal, 1 restricted

| Campo | Termo | classification | pii | sensivel | Referencia | Justificativa |
|---|---|---|---|---|---|---|
| `cep` | `endereco.cep` | `confidential` | sim | nao | LGPD art. 5, I | CEP de residencia nao identifica sozinho, mas no Brasil chega ao nivel de logradouro e reidentifica quando combinado com data de nascimento. |
| `cpf` | `pessoa.cpf` | `restricted` | sim | nao | LGPD art. 5, I | Identificador univoco de pessoa natural emitido pela Receita Federal. Identifica sozinho e serve de chave de cruzamento com praticamente qualquer outra base do pais. |
| `data_cadastro` | `cadastro.data_criacao` | `internal` | nao | nao | LGPD art. 5, I — por exclusao | Descreve o ciclo de vida do registro, nao a pessoa: isolada, nao identifica ninguem nem revela atributo de pessoa natural. Numa linha ja identificada ela herda o carater pessoal do conjunto — risco que se trata no nivel do dataset, nao no do campo. |
| `data_nascimento` | `pessoa.data_nascimento` | `confidential` | sim | nao | LGPD art. 5, I | Nao identifica sozinha, mas compoe o quase-identificador classico (data de nascimento + sexo + localidade), que reidentifica parcela alta de uma populacao quando combinado. |
| `email` | `contato.email` | `confidential` | sim | nao | LGPD art. 5, I | Endereco de contato individual. Na pratica funciona como identificador de conta e alcanca a pessoa diretamente. |
| `id_cliente` | `pessoa.identificador_interno` | `confidential` | sim | nao | LGPD art. 5, I c/c art. 13, par. 4 | Chave pseudonimizada. A pseudonimizacao nao descaracteriza o dado pessoal enquanto a organizacao mantiver, ainda que separada, a informacao adicional que permite reassociar a chave a pessoa. |
| `nome_completo` | `pessoa.nome_completo` | `confidential` | sim | nao | LGPD art. 5, I | Nome civil identifica a pessoa natural sem necessidade de cruzamento com outra base. |
| `segmento` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |
| `valor_total_compras` | — | **sem classificacao** | — | — | — | campo sem termo no glossario (lacuna de F2) — classificar exige decisao humana |

Os campos seguem o ODCS: `classification` e o campo do padrao, e `pii`/`sensivel` viram `tags: [pii, sensitive]` no contrato enriquecido que F4 vai propor.

Campo sem classificacao nao reprova F3: a cobertura exigida e de decisao, nao de acerto. Sem termo no glossario nao ha o que consultar no catalogo, e decidir isso e do humano — o gate e F4.
