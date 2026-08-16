# O laudo — como nasce, o que afirma, e o que o sustenta

O laudo é o entregável de governança deste projeto. O contrato classificado é
consequência; o laudo é o que responde, meses depois, *"por que este campo foi
classificado assim, e sob qual critério?"*.

---

## Como ele nasce

Da esteira, sempre. Nunca da máquina de quem escreveu o contrato.

```
contrato (YAML)
   │
   ├─ export jsonschema ──→ campos
   │                          │
   │       glossary/ ─────────┤  F2 · casa cada campo com um termo canônico
   │                          │
   │  classification/ ────────┤  F3 · o termo carrega classificação, PII,
   │                          │       base legal e justificativa
   │                          ↓
   └──────────────────────→ composição ──→ laudo.md
                                  │
                                  └──→ contrato enriquecido (classification por campo)
```

Não há inferência sobre nome de campo, e **nenhum modelo participa da decisão**.
A classificação é consulta a catálogo: dois campos que casam com o mesmo termo
recebem a mesma resposta em qualquer contrato, em qualquer data.

**Por que na esteira e não localmente:** o laudo é documento de governança, e o
que ele afirma não pode depender do que estava instalado na máquina de alguém.
Emitido sempre no mesmo ambiente, com a versão fixada em `harness.lock`, ele tem
uma única procedência possível.

---

## O que ele contém

**O cabeçalho** — contrato, versão e o **sha256 do contrato classificado**, o
arquivo que está no repositório ao lado dele. Quem auditar confere a
correspondência com qualquer ferramenta de hash, sem depender deste projeto.

**O critério aplicado** — glossário e catálogo, com versão e sha256 de cada.

**A tabela por campo** — termo, `classification`, PII, sensível, base legal e
justificativa. É a parte que uma pessoa lê.

**As pendências de decisão humana** — as lacunas, com o hash do pedido.

E duas coisas que ele **não** contém, de propósito:

**Data no corpo.** A data de emissão é a do commit, e o Git responde por ela
melhor. Sem data, o documento é determinístico: mesmo contrato, mesmo glossário
e mesmo catálogo produzem o mesmo arquivo byte a byte.

**Assinatura ou campo de aprovação.** Este é o laudo *técnico*. Quem assina é o
merge, e a revisão que o autorizou fica no histórico do repositório, presa ao
mesmo sha256 do cabeçalho. Um campo "aprovado por" dentro do arquivo seria uma
segunda verdade, mais fácil de forjar que o histórico.

---

## O nome do arquivo

```
1.0.0-2fcab96-4fc5b5f.md
 │      │        └── sha256 do critério (glossário + catálogo)
 │      └── sha256 do contrato classificado
 └── versão declarada no contrato
```

**Os três componentes são necessários.** A versão para uma pessoa achar; o sha
do contrato para não sobrescrever; e o sha do critério porque **o mesmo contrato
julgado por um catálogo novo é outra constatação**. Sem o terceiro, subir o
catálogo faria a segunda emissão cair sobre a primeira — e a auditoria perderia
exatamente o par que quer comparar.

Ao lado do `.md` ficam dois anexos com o mesmo nome-base: `.html` (o contrato
desenhado, para quem decide sobre o dado e não lê YAML) e `.proposta.json` (a
decisão em JSON, com o confronto entre o que o contrato **declarava** e o que o
catálogo **diz**, campo a campo — o único jeito de perguntar *"algum campo foi
rebaixado?"* em duzentos contratos).

---

## Por que ele é imutável na prática

O `check` recompara o laudo **byte a byte** contra o que ele próprio produziria.
Editar um laudo à mão reprova o pull request na hora, e a mensagem diz o que
diverge.

Isso é mais forte que permissão de arquivo: permissão se contorna com um commit;
verificação de conteúdo não, porque o conteúdo é determinado pelo contrato e
pelo critério — nenhum dos dois sob controle de quem edita o laudo.

Laudo emitido também nunca é sobrescrito. Se um arquivo com o mesmo nome existir
com conteúdo diferente, o `aplicar` **recusa** em vez de gravar: o nome carrega
contrato e critério, então divergência ali indica algo fora do lugar, e apagar a
constatação anterior destruiria a prova.

---

## O gate — quando o laudo diz "não sei"

Quando um campo não tem termo no glossário, o harness **não chuta**. Ele registra
uma lacuna, e o campo segue **sem classificação** no contrato: o harness não
escreve "não sei" num contrato de dados.

O laudo é emitido mesmo assim, listando as pendências — e é justamente esse
documento que o revisor precisa ver antes de aprovar. Esperar o gate fechar para
emitir deixaria o pull request sem o único artefato que descreve o que está sendo
decidido.

O veredito nesse caso é `5`, e **o job fica verde**: nada está errado no
contrato, falta decisão humana. Quem segura o merge é a revisão de CODEOWNER.

Uma lacuna se resolve de dois jeitos, e os dois são decisão registrada: ampliar o
glossário e o catálogo (num repositório que o autor do contrato não controla), ou
aceitar que o campo siga sem classificação.

---

## O limite que você precisa conhecer

**O laudo só afirma sobre os campos que o harness enxergou** — e hoje ele enxerga
apenas o primeiro nível do contrato. Num documento aninhado (MongoDB, JSON
estruturado), os campos internos ficam invisíveis, e o laudo **não menciona que
existem**.

Isso está medido e descrito em [`cobertura.md`](cobertura.md). É o limite mais
importante deste projeto hoje, e quem for usar o laudo como prova de conformidade
precisa saber dele antes.
