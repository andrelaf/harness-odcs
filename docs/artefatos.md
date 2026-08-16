# Pacote, container e artefatos

Três palavras que aparecem o tempo todo neste projeto e significam coisas
diferentes. Este documento separa as três e lista **tudo** que é produzido, com
onde vive e por quanto tempo.

---

## Container ≠ pacote

**O container é o motor de validação.** É o
[`datacontract-cli`](https://github.com/datacontract/datacontract-cli), imagem
`datacontract/cli:1.1.0`, fixada **por digest** em
[`scripts/env.sh`](../scripts/env.sh). Ele é software de terceiros que este
projeto consome, nunca produz.

O harness o chama quatro vezes por verificação:

| Chamada | Para quê |
|---|---|
| `lint <contrato>` | O contrato é ODCS sintaticamente válido? |
| `export jsonschema` | **Extrair os campos** — é assim que o harness sabe o que existe |
| `lint <enriquecido>` | O contrato já classificado continua válido? |
| `export html` | O desenho para quem não lê YAML |

Repare no que **não** está na lista: nada de classificar. O CLI não sabe o que é
LGPD, glossário ou dado pessoal. Ele responde *"isto é um contrato válido, e
quais campos ele tem?"* — e só. A classificação é Rust puro lendo dois YAMLs;
nenhum container participa dela.

O harness **não faz parsing de ODCS**, de propósito. Ler
`schema[].properties[].name` do YAML parece trivial até o primeiro contrato com
propriedade aninhada, e aí existiriam duas interpretações do padrão no
repositório — a segunda sendo a errada.

O container só enxerga a raiz de dados: `datacontract_args` monta apenas
`HARNESS_ROOT`. **O vocabulário nunca entra no container.**

---

**O pacote é o harness distribuído.** É o que este projeto produz para ser
consumido por um repositório de contratos: um tarball com o binário, os scripts
e o vocabulário.

```
harness-odcs/
  harness.sh                 entrypoint de quem usa — não compila nada
  bin/harness-odcs           o binário, em release
  scripts/env.sh             a configuração, fonte única
  scripts/imagem.sh          garante a imagem do motor no digest fixado
  glossary/                  o critério
  classification/            o critério
  VERSION                    procedência: versão, commit, sha256 do vocabulário
```

Montado por [`scripts/package.sh`](../scripts/package.sh), publicado por tag, e
fixado no repositório de contratos **por versão e sha256** em `harness.lock`.
Detalhes em [`distribuicao.md`](distribuicao.md).

A distinção em uma frase: **o container valida a sintaxe; o pacote carrega o
julgamento.**

---

## Todo artefato produzido

### Efêmero — vive num run, não vai para o repositório

`evidence/<run_id>/`, regenerável a partir do contrato e do critério:

| Arquivo | O que é |
|---|---|
| `01-check-lint.txt` … `04-check-html.txt` | stdout e stderr brutos de cada chamada ao container |
| `f1-lint.json` | relatório de lint do contrato-fonte |
| `f4-campos-check.json` | o JSON Schema exportado — de onde saem os campos |
| `f4-contrato-enriquecido.odcs.yaml` | o contrato **proposto**, com `classification` por campo |
| `f4-lint-enriquecido.json` | lint do enriquecido |
| `f4-proposta.json` | a decisão completa, campo a campo |
| `f4-contrato.html` | o desenho, antes de normalizado |
| `laudo.md`, `laudo.html`, `laudo.proposta.json` | os documentos propostos |
| `report.json` | **o contrato de saída** — veredito, defeitos, gate, propostas |

`trace/<run_id>.jsonl` — uma linha por transição, append-only, com `duration_ms`
e `exit_code`. É dele que a medição é derivada; não há contador paralelo.

No pipeline, `evidence/` e `trace/` sobem como **artefato do job**, com retenção
de 30 dias. No repositório de contratos eles são ignorados pelo `.gitignore`.

### Permanente — versionado ao lado do contrato

Três arquivos, mesmo nome-base, emitidos pela esteira:

```
contracts/<dominio>/<contrato>/laudos/
  1.0.0-2fcab96-4fc5b5f.md             o laudo, para quem revisa e audita
  1.0.0-2fcab96-4fc5b5f.html           o contrato desenhado, para quem não lê YAML
  1.0.0-2fcab96-4fc5b5f.proposta.json  a decisão, para consulta automatizada
```

O nome carrega **versão do contrato + sha256 do contrato + sha256 do critério**.
Reclassificar o mesmo contrato com um catálogo novo cria um laudo *ao lado*, não
*por cima* — e é esse par que uma auditoria quer comparar.

E o próprio `contract.odcs.yaml`, que ganha `classification` por campo.

### O critério de corte: determinismo

Só é versionado o que produz **os mesmos bytes para a mesma entrada**. Foi por
isso que:

- **`.lint.json` saiu.** Eram 659 bytes com 14 de 17 campos `null`, dizendo
  `"result": "passed"`. Recibo, não auditoria.
- **O HTML precisou ser normalizado.** O `export html` carimba
  `Created at <data> UTC` no rodapé — duas execuções seguidas dão arquivos
  diferentes. A hora foi trocada pela procedência (`Gerado do contrato sha256
  03d0120f…`): a data de emissão já é a do commit, e o Git responde por ela
  melhor que um rodapé.
- **O Excel ficou de fora.** É binário e muda a cada execução a ponto de o
  tamanho variar (28906 → 28907 bytes). No git vira blob reescrito inteiro, sem
  diff legível. Gerar sob demanda continua sendo o certo para ele.

Sem determinismo, o `check` não conseguiria **exigir** o arquivo — e é essa
exigência que impede um pull request de ser aprovado com o laudo só na tela.

---

## O `report.json`, e por que ele existe

É o contrato de saída do `check`: `schema_version`, veredito, exit code,
defeitos com arquivo e etapa, itens de gate, e onde estão as propostas.

Ele existe para que **a plataforma de CI seja trocável**. GitHub e Azure DevOps
consomem o mesmo arquivo; o que muda entre os dois é um renderizador de ~30
linhas. Sem ele no meio, cada plataforma leria o texto do console e a
portabilidade viraria trabalho de expressão regular.

É também o que permite executar uma vez e desenhar três: anotação no diff,
comentário no PR e resumo do job saem todos do mesmo arquivo gravado. **99,6% do
custo medido é partida de container** — desenhar três vezes não pode custar três
verificações.
