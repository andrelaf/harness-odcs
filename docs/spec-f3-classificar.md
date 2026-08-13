# F3 — Classificar

> Spec curta da feature. O contrato do harness está em
> [`spec-harness.md`](spec-harness.md) e vence em qualquer conflito.

**O que a feature entrega:** cada campo do contrato sai com uma classificação de
privacidade — `classification`, `pii`, `sensivel`, justificativa e referência
legal —, ou marcado como `nao_classificado`, com PASS/FAIL explícito sobre a
cobertura.

O que ela **não** entrega: o relatório de lacunas, a detecção de reclassificação
sensível e a pausa para aprovação humana. Isso é F4, e F3 produz o insumo dos
três sem antecipar nenhum. Também **não escreve no contrato**, pelo mesmo motivo
de F2: persistir enriquecimento é decisão sujeita ao gate.

---

## Entradas

```
contracts/clientes/contract.odcs.yaml     o contrato
glossary/glossario.yaml                   o glossário canônico
classification/catalogo-lgpd.yaml         o catálogo de privacidade
```

Vocabulário, regras de coerência e política de versão do catálogo:
[`classification/README.md`](../classification/README.md).

## Os campos são os do ODCS, não inventados

Decisão tomada lendo `odcs-3.1.0.schema.json`, empacotado no
`datacontract-cli` 1.1.0. A seção que o editor rotula **"Classification &
Security"** corresponde a exatamente três campos de propriedade:
`classification` (string livre), `criticalDataElement` (bool) e `encryptedName`
(string). **Não existe campo `pii` no ODCS 3.1.0** — a forma nativa de marcar
dado pessoal é por tag, e a orientação embutida no próprio editor é literalmente
`tags: ["pii", "sensitive"]`.

Então uma entrada do catálogo é:

```yaml
- termo: pessoa.cpf
  classification: restricted     # o campo ODCS, com vocabulário controlado
  pii: true                      # -> tags: [pii]
  sensivel: false                # -> tags: [sensitive]
  justificativa: ...
  referencia: LGPD art. 5, I
```

A consequência prática é o que justifica a escolha: o enriquecimento que F4 vai
propor é **ODCS válido por construção** — `classification` cai no campo homônimo,
`pii`/`sensivel` viram `tags`, e o vínculo com o glossário cabe em
`authoritativeDefinitions` com `type: businessDefinition`. Um vocabulário
próprio obrigaria a inventar uma tradução na hora de escrever no contrato, que é
exatamente onde esse tipo de coisa se perde.

`criticalDataElement` e `encryptedName` ficam de fora de propósito: o primeiro é
criticidade de negócio (decisão do data owner, e varia por dataset, não por
termo) e o segundo nomeia uma coluna concreta que um catálogo chaveado por termo
não tem como conhecer.

Uma versão anterior desta spec tinha duas dimensões próprias — categoria
jurídica e grau de identificação. Foram substituídas: a escala de sensibilidade
do `classification` **já é** o gradiente de risco, dito num campo que o padrão
conhece.

## Evidência é saída, nunca entrada

F3 precisa do mapeamento campo→termo que F2 produz. Ele **não** é lido de
`evidence/<run_id>/f2-mapeamento.json`: F3 chama
`f2_mapear::mapeamento_atual` e recomputa a partir do contrato e do glossário.

Ler o artefato de um run anterior exigiria descobrir *qual* run foi o último de
F2 — estado que o harness não guarda — e amarraria o resultado de hoje a um
arquivo que pode ter sido produzido por outro contrato, outro glossário ou outra
versão do binário. Recomputando, as garantias de F2 (integridade do glossário,
cobertura de decisão) valem dentro de F3 de graça, e a cadeia
contrato → termo → classificação é reconstruída inteira a cada run.

O contrato, por ser lido por F2, F3 e F4, mora em `features::contrato` — não
dentro de F2.

## A classificação mora no termo, não no campo

É o retorno de F2 sendo cobrado. `cpf`, `nr_cpf` e `documento_cpf` casam com
`pessoa.cpf`, e o catálogo classifica `pessoa.cpf` — então os três recebem a
mesma resposta, em qualquer contrato, em qualquer semana. Classificar por nome
de campo recriaria o problema de consistência que o glossário existe para
resolver.

Consequência direta: **campo sem termo não tem como ser classificado.** Uma
lacuna de F2 vira `nao_classificado` em F3 e segue para o humano — que é
exatamente o comportamento pedido para o caso ambíguo do `contexto.md`.

## Nenhum modelo decide nada aqui, também

A classificação de um campo é uma consulta: termo → entrada do catálogo. Não há
inferência, não há heurística sobre nome de campo, não há LLM. O que o código
não encontra no catálogo, ele não inventa — nomeia como não classificado e
encaminha.

## Divisão entre as fases

| Fase | Faz |
|---|---|
| `implement` | Recompõe o mapeamento, carrega o catálogo, classifica e grava `evidence/<run_id>/f3-classificacao.json`. |
| `verify` | **Refaz tudo do zero**, confere integridade do catálogo e cobertura, devolve PASS/FAIL e grava `f3-cobertura.json` e `f3-classificacao.md`. |

Mesmo desenho de F1 e F2: `verify` não lê a proposta como insumo, o que mantém
`./run.sh verify` válido sozinho e dá de graça a comparação byte a byte entre as
duas fases — divergência só pode significar entrada alterada no meio do run. O
artefato não carrega `run_id` nem timestamp, e é isso que torna a comparação
possível e dois runs comparáveis com `diff`.

## Regra de PASS

`verify` passa quando **todas**:

1. o catálogo é íntegro — regras em
   [`classification/README.md`](../classification/README.md#integridade--o-que-torna-o-catálogo-inválido),
   incluindo **cobrir o glossário inteiro**;
2. a cobertura é total: cada campo do contrato aparece exatamente uma vez, e
   nenhum campo inventado;
3. cada campo classificado **bate com a entrada do catálogo** do seu termo —
   `classification`, `pii`, `sensivel` e referência vêm de lá, não de lugar
   nenhum;
4. `nao_classificado` não carrega nenhum desses campos, e toda decisão traz
   justificativa não vazia;
5. os totais do resumo batem com as decisões;
6. quando `implement` rodou no mesmo run, a recomputação bate com a proposta.

O item 3 é o que impede F3 de ser só uma contagem: ele verifica que a
classificação **veio do catálogo**, e não de uma decisão que apareceu no meio do
caminho. É a trava do `contexto.md` — "nunca decide sozinho o que é persistido"
— escrita como asserção.

**Campo `nao_classificado` não é FAIL**, pela mesma razão de F2: cobertura total
aqui é de decisão, não de acerto. O que o harness recusa é campo que atravessou
o fluxo sem ninguém dizer nada sobre ele.

### Duas ausências que parecem iguais e não são

| Situação | Veredito | Por quê |
|---|---|---|
| Campo do contrato sem termo no glossário | lacuna, PASS | O vocabulário não cobre o mundo; decidir isso é do humano, em F4 |
| Termo do glossário sem entrada no catálogo | **defeito, FAIL** | O catálogo tem obrigação de cobrir o vocabulário; a falta é manutenção atrasada, não ambiguidade |

Tratar as duas como lacuna deixaria o catálogo apodrecer em silêncio: bastaria
não classificar um termo novo para que os campos dele saíssem "pendentes de
decisão humana" para sempre, sem que nada acusasse a origem real do problema.

### Catálogo quebrado para em `implement`

Entrada malformada é falha de preparação, não veredito — a mesma linha de F1 e
F2. As duas fases chamam a mesma função de integridade, então não há como
divergirem.

## Evidência produzida

| Arquivo | Fase | O que é |
|---|---|---|
| `f3-campos-implement.json` / `f3-campos-verify.json` | ambas | saída crua do `export jsonschema`, uma por fase |
| `f3-classificacao.json` | `implement` | a proposta: termo, `classification`, `pii`, `sensivel`, justificativa e referência por campo |
| `f3-cobertura.json` | `verify` | o veredito, com contagens por nível e defeitos encontrados |
| `f3-classificacao.md` | `verify` | a tabela que uma pessoa lê sem ferramenta |

Nenhum carrega valor de dado — só nome de campo, tipo, identificador de termo e
decisão. A trave de privacidade da spec do harness (seção 6) vale sem exceção.

## Funções puras e testes

`carregar_catalogo`, `defeitos_do_catalogo`, `classificar` e
`conferir_cobertura` são puras. É o que permite exercitar catálogo incoerente,
termo sem classificação, classificação divergente do catálogo e cobertura furada
em `cargo test`, sem subir container.

## Onde a implementação mora

`src/features/f3_classificar.rs`, compilada, registrada em
`features::dispatch`. Mesmo motivo de F1 e F2.
