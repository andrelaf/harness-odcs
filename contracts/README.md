# contracts/ — convenção de layout

Um diretório por contrato. O nome do diretório é a identidade do contrato; o
nome do arquivo é o papel dele.

```
contracts/
  clientes/
    contract.odcs.yaml         a fonte — único arquivo escrito à mão
    laudos/
      1.0.0-29e5a60.md         o laudo emitido para aquele conteúdo
  <outro-contrato>/
    contract.odcs.yaml
```

## Por que diretório e não `<nome>.odcs.yaml` na raiz

Porque o validador não produz só um veredito: `datacontract export html` gera
relatório, `lint --output` gera resultado em JSON, e outros comandos geram
schema, DDL e diagrama. Com os contratos soltos na raiz, o segundo contrato já
mistura arquivo de origem com arquivo derivado, e não dá mais para olhar o
diretório e saber o que é entrada humana.

## Onde vai o que a ferramenta gera

**Não vai para cá.** Saída de ferramenta é evidência de execução e vive em
`evidence/<run_id>/`, junto do resto do rastro daquele run — é a regra da
[spec do harness](../docs/spec-harness.md), seção 6.

A exceção, e o critério que a separa: **`laudos/`**. O que distingue os dois não
é o formato, é se o artefato é *regenerável* ou *emitido*. O relatório HTML, o
DDL e o Excel se refazem idênticos a partir do contrato — some hoje, volta
amanhã, e por isso são evidência. O laudo é registro: diz contra qual versão de
glossário e de catálogo aquele contrato foi classificado, com a base legal de
cada campo. Auditoria não aceita "conseguimos regerar"; pede o documento.

Por isso ele fica versionado, ao lado do contrato, e entra no mesmo commit que a
classificação a que se refere — nomeado por versão do contrato **e** sha256 do
contrato classificado, para que um laudo nunca apague o anterior. O detalhe está
em [`spec-f4-gate.md`](../docs/spec-f4-gate.md#o-laudo-é-o-entregável-e-não-é-evidência).

A consequência é deliberada: o mesmo contrato validado em dois runs produz dois
relatórios, lado a lado, comparáveis. Se o relatório morasse aqui, o segundo run
sobrescreveria o primeiro e a comparação — que é justamente o que se quer
auditar — desapareceria.

O `.gitignore` recusa `*.html` dentro de `contracts/` como rede de segurança
para um `--output` distraído. A regra não é o `.gitignore`; a regra é esta
página. O `.gitignore` só evita que o engano vire commit.

## Restrição de conteúdo

Domínio sintético. Nenhum dado real ou de produção entra aqui — nem em exemplo,
nem em comentário. O contrato descreve **metadados**: nomes de campo, tipos e
descrições.
