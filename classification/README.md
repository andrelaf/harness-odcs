# classification/ — o catálogo de privacidade

Um arquivo, `catalogo-lgpd.yaml`, com a classificação LGPD de cada termo
canônico. É a entrada de domínio de
[F3 — Classificar](../docs/spec-f3-classificar.md).

## Por que é um arquivo separado do glossário

Porque são dois donos com cadências diferentes. O glossário é do data steward:
o que cada termo significa. O catálogo é do encarregado de dados: o que a lei
diz sobre esse termo. Fundidos num arquivo só, uma revisão jurídica passaria a
versionar o vocabulário inteiro, e a pergunta de auditoria — *qual catálogo
classificou este campo?* — não teria resposta própria.

Cada um tem sua `version`, e as duas entram em todo artefato de classificação.

## Por que é chaveado por termo, e não por campo

É o retorno de F2. `cpf`, `nr_cpf` e `documento_cpf` casam com `pessoa.cpf`, e
a classificação mora no termo — então os três recebem a mesma resposta, sempre.
Classificar por nome de campo recriaria exatamente o terceiro problema descrito
no [README](../README.md#o-problema): a mesma coisa, classificada em semanas
diferentes, virando respostas diferentes.

## Os campos falam ODCS, não um vocabulário próprio

Esta é a decisão que mais importa aqui, e ela foi tomada **lendo o schema**, não
de memória. Em `odcs-3.1.0.schema.json`, empacotado no `datacontract-cli`
1.1.0, uma propriedade de schema tem exatamente três campos na seção que o
editor rotula **"Classification & Security"**:

| Campo ODCS | Tipo | O que o padrão diz |
|---|---|---|
| `classification` | string livre | *"Can be anything, like confidential, restricted, and public to more advanced categorization."* |
| `criticalDataElement` | boolean | se o elemento é um CDE |
| `encryptedName` | string | nome da coluna que guarda o valor cifrado |

**Não existe campo `pii` no ODCS 3.1.0.** A forma nativa de marcar dado pessoal
é por tag — a orientação embutida no próprio editor é literalmente
`tags: ["pii", "sensitive"]`.

Daí a forma de uma entrada:

```yaml
- termo: pessoa.cpf
  classification: restricted     # o campo ODCS, com vocabulário controlado
  pii: true                      # -> tags: [pii]
  sensivel: false                # -> tags: [sensitive]
  justificativa: ...             # exigida pelo contexto.md, por decisão
  referencia: LGPD art. 5, I     # a base legal da decisão
```

Escrever o catálogo assim tem uma consequência prática: o enriquecimento que F4
vai propor é **ODCS válido por construção** — `classification` cai no campo
homônimo, `pii`/`sensivel` viram `tags`, e o vínculo com o glossário cabe em
`authoritativeDefinitions` com `type: businessDefinition`. Um vocabulário
próprio obrigaria a inventar uma tradução na hora de escrever no contrato, que é
onde esse tipo de coisa se perde.

### Uma dimensão, não duas

Uma versão anterior deste catálogo tinha dois eixos: categoria jurídica
(`pessoal`/`pessoal_sensivel`/`nao_pessoal`) e grau de identificação
(`direta`/`indireta`/`nenhuma`). Os dois foram substituídos por
`classification` + as duas tags, porque a escala de sensibilidade **já é** o
gradiente de risco: `cpf` é `restricted`, `data_nascimento` é `confidential`, e
a diferença entre identificação direta e indireta está dita aí, num campo que o
padrão conhece.

### `classification` — vocabulário controlado

O padrão deixa o campo livre. Aqui ele é fechado nestes quatro valores, na
ordem dos exemplos do próprio ODCS:

| Valor | Quando |
|---|---|
| `public` | Divulgável sem restrição |
| `internal` | Circula na organização; não é dado pessoal |
| `confidential` | Dado pessoal |
| `restricted` | Dado pessoal sensível, ou identificador nacional cujo vazamento é irreversível |

Fechar o vocabulário é decisão nossa, não do padrão: string livre em campo de
classificação vira `Confidential`, `confidencial` e `CONF` em três semanas.

### `criticalDataElement` e `encryptedName` ficam de fora

Os dois estão na mesma seção do editor e **não** entram no catálogo, de
propósito. `criticalDataElement` é criticidade de negócio — quem decide é o
data owner, não o encarregado de dados, e a resposta muda por dataset, não por
termo. `encryptedName` nomeia uma coluna concreta de um dataset concreto; um
catálogo chaveado por termo não tem como saber. Colocar os dois aqui seria dar
ao catálogo autoridade que ele não tem.

## Integridade — o que torna o catálogo inválido

Verificado a cada run, e um FAIL de fase:

- entrada sem `termo`, sem `justificativa` ou sem `referencia`;
- dois termos iguais no catálogo;
- entrada apontando para termo que não existe no glossário;
- **termo do glossário sem entrada no catálogo**;
- incoerência entre os campos:
  - `sensivel: true` sem `pii: true` — dado sensível é dado pessoal;
  - `sensivel: true` com `classification` diferente de `restricted`;
  - `pii: true` com `classification: public`;
  - `pii: false` com `classification: restricted` — restringir por outro motivo
    (segredo comercial, por exemplo) não é decisão de um catálogo de privacidade.

A quarta regra é a que mais importa: o catálogo tem de **cobrir o glossário
inteiro**, e isso é verificável sem nenhum contrato. Termo sem classificação
significa catálogo defasado em relação ao vocabulário — acoplamento desejado,
porque acrescentar termo passa a obrigar a classificá-lo. É a diferença entre
uma lacuna (campo que o vocabulário não cobre, decisão humana, F4) e um defeito
(vocabulário que o catálogo não cobre, manutenção atrasada, FAIL agora).

> `sensivel: true` não é usado por nenhum termo do vocabulário atual — não há
> campo de saúde, biometria ou convicção no contrato de clientes. A tag existe
> porque F4 depende dela como gatilho e porque um vocabulário que cresce vai
> encontrá-la. Vale dizer isso em voz alta em vez de deixar parecer que a
> classificação foi exercitada em todos os valores.

## Versão

| Mudança | Como versionar |
|---|---|
| Classificar termo novo | *minor* — nada que já valia muda |
| Corrigir texto de `justificativa` ou `referencia` | *patch* |
| Mudar `classification`, `pii` ou `sensivel` de um termo existente | *major* |

A terceira linha é a **reclassificação** de que fala o `contexto.md`: mudar a
classificação de um termo já classificado altera o veredito de todo contrato que
o usa, retroativamente. Um *major* aqui não é burocracia — é o sinal de que a
mudança precisa passar pelo gate humano de F4 antes de valer.
