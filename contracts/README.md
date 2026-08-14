# contracts/ — convenção de layout

Um diretório por contrato. O nome do diretório é a identidade do contrato; o
nome do arquivo é o papel dele.

```
contracts/
  <domínio>/                   clientes, pagamentos, credito — recomendado
    <contrato>/                = ao `id` declarado no contrato
      contract.odcs.yaml       a fonte — único arquivo escrito à mão
      laudos/
        1.0.0-03d0120.md       o laudo emitido para aquele conteúdo
```

## A convenção de nome, e por que ela é verificada

Com um contrato, o caminho é decoração. Com duzentos, ele é o índice: é por ele
que uma pessoa acha o contrato, e é por ele que o `CODEOWNERS` roteia a revisão
para quem responde por aquele dado. Nome fora do padrão quebra as duas coisas em
silêncio, e só aparece quando alguém precisa achar o contrato às pressas.

Por isso **F1 verifica o nome**, e não só o schema. As regras:

| Regra | Por quê |
|---|---|
| Começa em `contracts/` | é onde o harness lê e **escreve** |
| Termina em `contract.odcs.yaml` | o nome do contrato é o do diretório; o do arquivo é o papel dele |
| Um diretório por contrato | o laudo mora ao lado — solto em `contracts/` não há "ao lado" |
| No máximo `<domínio>/<contrato>/` | mais fundo que isso nenhuma ferramenta consegue prever |
| Segmentos em kebab-case minúsculo | maiúscula, acento, espaço e `_` são quatro jeitos de escrever o mesmo nome de quatro formas |
| **`id` do contrato = nome do diretório** | senão o nome que as ferramentas usam não é o que está no caminho |

O nível de **domínio** é o único opcional: sem ele o fluxo passa, com aviso. Ele
existe por razão mecânica, não estética — é o que permite ao `CODEOWNERS` dar a
revisão ao time dono do dado. Sem ele, ou uma pessoa aprova tudo, ou o arquivo
lista contrato por contrato.

A última regra é a que mais paga. Foi ela que pegou, neste repositório, um
`id: clientes-sintetico` morando em `contracts/clientes/` — divergência que
passou por quatro semanas sem ninguém notar.

## Qual contrato o harness opera

Com **um** contrato no repositório, nenhum: ele resolve sozinho. Com **dois ou
mais**, a escolha passa a ser obrigatória, e o harness recusa e lista em vez de
adivinhar:

```
$ ./run.sh verify
erro: 2 contratos no repositorio — escolha um com `--contrato`:
  contracts/clientes/contract.odcs.yaml
  contracts/pagamentos/transacoes/contract.odcs.yaml
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
