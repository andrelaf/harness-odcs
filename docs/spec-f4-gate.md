# F4 — Gate + relatório de lacunas

> Spec curta da feature. O contrato do harness está em
> [`spec-harness.md`](spec-harness.md) e vence em qualquer conflito.

**O que a feature entrega:** o contrato enriquecido com a classificação de F3,
o relatório do que ficou sem decisão, e a **pausa** — nada é persistido no
contrato enquanto houver pendência que o harness não tem autoridade para
resolver sozinho.

É a feature que fecha o critério de sucesso do [`contexto.md`](contexto.md):
*"o contrato enriquecido, a classificação de privacidade de todos os campos e
um relatório de lacunas"*, com *"nenhum campo PII sem controle"*.

---

## Entradas

```
contracts/clientes/contract.odcs.yaml     o contrato — e também o destino
glossary/glossario.yaml                   via F2, recomputado
classification/catalogo-lgpd.yaml         via F3, recomputado
state/aprovacoes.json                     o que o humano já liberou
```

F4 **não lê** `evidence/` de run anterior, pelo mesmo motivo de F3: chama
`f3_classificar::classificar` sobre um mapeamento recomputado, e a cadeia
contrato → termo → classificação → enriquecimento é reconstruída inteira a cada
run. Evidência é saída, nunca entrada.

## O contrato é a única fonte da decisão anterior

Esta é a decisão central da feature, e ela define o que o gate consegue
proteger.

O harness não guarda "o que eu classifiquei da última vez". Ele lê **o que o
contrato declara hoje** — `classification`, `tags: [pii, sensitive]` — e compara
com o que o catálogo diz agora. O que já está no arquivo é a decisão humana
vigente, porque foi ela que passou por commit e revisão.

A consequência é a que interessa: uma mudança de `classification`, `pii` ou
`sensivel` no catálogo — o *major* previsto em
[`classification/README.md`](../classification/README.md#versão) — aparece
sozinha, no run seguinte, como divergência entre a proposta e o contrato. Não
existe "esqueci de avisar o gate": o gatilho não depende de ninguém registrar a
intenção da mudança, só de o arquivo estar diferente do que a lei diz.

## O que abre o gate

Três situações, e nenhuma delas é resolvida pelo harness:

| Tipo | Quando | Por quê é humano |
|---|---|---|
| `lacuna` | Campo sem classificação — F2 não achou termo | O vocabulário não cobre o campo; ampliar o glossário ou aceitar o campo sem classificação é decisão de quem responde pelo dado |
| `reclassificacao` | O contrato já declara algo, e a proposta diverge | Sobrescrever decisão humana anterior — em qualquer direção — é exatamente o que o `contexto.md` proíbe fazer sozinho |
| `conflito` | O contrato declara classificação num campo que o catálogo não sabe classificar | O harness não consegue reproduzir a decisão que está no arquivo; apagá-la ou mantê-la é escolha de quem a tomou |

**Campo sem nada declarado não abre gate.** É primeira classificação, não
reclassificação: ninguém está sendo sobrescrito, e o valor vem de um catálogo
que já é um artefato humano, com justificativa e referência legal por entrada.
Fazer o gate disparar aqui o transformaria em pedido de confirmação de rotina —
e gate que sempre dispara é gate que ninguém lê.

### Por que lacuna também para o fluxo

O `contexto.md` nomeia a pausa falando de reclassificação sensível. Mas lista,
entre as falhas previsíveis, *"encerrar com o relatório incompleto"*, com a
trava correspondente: *"exigência de que tudo esteja classificado antes de
concluir"*.

F2 e F3 empurraram cada campo sem decisão para cá — "o gate é F4", nas duas
specs. Se F4 apenas escrevesse esses campos num relatório e concluísse, a trava
não existiria em lugar nenhum do fluxo, e o campo ambíguo do primeiro
experimento atravessaria o harness inteiro sem ninguém dizer nada sobre ele.

Aprovar uma lacuna é uma decisão de verdade — *"aceito que este campo siga sem
classificação"* — e fica registrada como tal.

## A aprovação vale para um conteúdo, não para uma feature

`./run.sh approve f4-gate` não libera "a feature f4". Ele libera **o pedido de
gate que estava pendente**, identificado pelo sha256 do conjunto de itens.

Isso importa quando alguém aprova, edita o contrato ou o catálogo, e roda de
novo: os itens mudam, o hash muda, e o gate fecha outra vez. Uma aprovação
carimbada na feature seria um passe permanente — aprovaria hoje uma lacuna em
`segmento` e amanhã, em silêncio, a despromoção de `cpf` de `restricted` para
`public`.

O ciclo tem duas metades, e cada uma mora onde deve:

```
F4 implement  ->  state/gate-pendente.json   (o pedido, com o hash)
./run.sh approve f4-gate  ->  state/aprovacoes.json   (o deferimento)
```

`approve` continua burro: ele não recomputa classificação nenhuma, só consome o
pedido que a feature deixou e o arquiva com data e run. Toda a política de o
que exige gate está em Rust, na feature.

## O enriquecimento é escrito no contrato, e só depois do veredito

`contracts/` deixa de ser somente fonte para este arquivo: o contrato
enriquecido é o entregável do projeto, e ele nasce onde o contrato mora, entra
no commit do `handoff` e vai para revisão como diff.

A escrita acontece em `verify`, **depois** do PASS, e nunca em `implement`:

- `implement` propõe, em `evidence/`. Se há gate, devolve `Blocked` — e o
  contrato não é tocado. A evidência é escrita **antes** da decisão do gate, de
  propósito: quem aprova precisa ler a proposta inteira, não só a lista de itens.
- `verify` julga — inclusive rodando `datacontract lint` **sobre a proposta** —
  e só então aplica.

Um `implement` que escrevesse no contrato antes do julgamento deixaria o
repositório num estado que nenhuma fase aprovou, e um `Blocked` teria de saber
desfazer o que escreveu.

O que é escrito em cada propriedade classificada:

```yaml
- name: cpf
  logicalType: string
  description: Cadastro de Pessoa Fisica, 11 digitos, sem formatacao.
  required: true
  classification: restricted
  tags:
    - pii
  authoritativeDefinitions:
    - url: glossary/glossario.yaml#pessoa.cpf
      type: businessDefinition
```

Os três campos existem no ODCS 3.1.0 e foram conferidos no
`odcs-3.1.0.schema.json` empacotado no CLI: `classification` é campo de
`SchemaBaseProperty`; `tags` e `authoritativeDefinitions` vêm de
`SchemaElement`, e o item de `authoritativeDefinitions` exige `url` e `type`,
com `additionalProperties: false`. Nada aqui é vocabulário inventado — é o que
[`classification/README.md`](../classification/README.md#os-campos-falam-odcs-não-um-vocabulário-próprio)
prometeu ao escolher a forma das entradas do catálogo.

Campo sem classificação **não é tocado**. O harness não escreve "não sei" no
contrato.

### O que a reescrita custa

O YAML é reserializado inteiro, então **comentários e linhas em branco do
contrato se perdem**. É consequência de usar um parser de YAML em vez de editar
texto por posição, e a alternativa — costurar linhas na mão para preservar
formatação — quebra no primeiro contrato com indentação diferente.

Vale dizer em voz alta em vez de descobrir no primeiro diff. O que a
reserialização preserva é o que importa: ordem das chaves e das propriedades, e
todo campo que já estava lá.

### Idempotência

Aplicar o enriquecimento duas vezes produz o mesmo arquivo. É o que permite
rodar F4 de novo sem gerar diff, e o que faz o gate ficar quieto quando nada
mudou: o contrato passa a declarar exatamente o que o catálogo diz, e não há
divergência para reportar.

## O laudo é o entregável, e não é evidência

O contrato enriquecido carrega a **conclusão** — `classification`, `tags` — e
não carrega o critério. Um contrato classificado não responde *por que* o campo
recebeu aquele nível, contra qual versão de catálogo, nem o que ficou em aberto.
Quem responde isso é o laudo, e ele é o documento que o time de governança de
dados hoje escreve à mão.

Por isso a base legal atravessa a cadeia inteira: `justificativa` e `referencia`
nascem no catálogo, F3 as carrega em `CampoClassificado`, e F4 as leva até
`CampoDoGate` sem tocá-las. O harness **transporta** a redação do encarregado de
dados; não redige nem reescreve nenhuma delas.

### Onde ele mora, e por quê

```
contracts/clientes/
  contract.odcs.yaml            a fonte, e o destino do enriquecimento
  laudos/
    1.0.0-29e5a60.md            um laudo por (versão, sha256 do classificado)
```

Isto é uma **exceção deliberada** à regra da seção 6 da spec do harness, e a
exceção tem critério: saída de ferramenta é *regenerável* — apago o HTML hoje,
refaço amanhã, sai idêntico —, e por isso vive em `evidence/<run_id>/`. O laudo
é *registro emitido*: tem critério vigente na data, e alguém responde por ele.
Auditoria não aceita "conseguimos regerar"; pede o documento. Ele fica
versionado, ao lado do contrato a que se refere, e entra no mesmo commit.

O nome leva **versão do contrato + sha256 do contrato classificado**. A versão
para uma pessoa achar; o sha para não sobrescrever. Dois laudos da mesma
`version` existem de verdade — a mesma versão reclassificada por um catálogo
novo são duas constatações diferentes, e apagar a primeira destruiria justamente
o que a auditoria quer comparar.

O sha é o do contrato **classificado**, não o da entrada: é o arquivo que fica
no repositório ao lado do laudo, então quem audita confere a correspondência com
qualquer ferramenta de hash, sem depender deste projeto.

### O que o laudo não tem

**Data no corpo.** O documento é determinístico: mesmo contrato, mesmo glossário
e mesmo catálogo produzem o mesmo arquivo byte a byte — reemitir nunca gera
diff, e `verify` pode conferi-lo como confere todo o resto. A data de emissão é
a do commit, e o Git já é a autoridade de tempo aqui. Um carimbo no corpo faria
o arquivo mudar sem que a análise tivesse mudado.

**Aprovação.** O laudo é a constatação técnica; quem assina é o merge. A revisão
que o autorizou fica no histórico, presa ao mesmo sha256 do cabeçalho.

### A base legal não entra no hash do gate

`hash_do_gate` continua sendo calculado só sobre os itens de gate. A
justificativa e a referência ficam de fora de propósito: o pedido é sobre **o
que** se decide, não sobre a redação do porquê. Se entrassem, corrigir uma
vírgula na justificativa de um termo invalidaria aprovações que ninguém pediu
para revisar.

### Ordem de escrita

Contrato primeiro, laudo depois. O laudo carrega o sha256 do contrato
classificado, então a ordem decide como fica o repositório se a segunda escrita
falhar: contrato escrito e laudo faltando é uma ausência — visível, e o run
seguinte emite. Laudo escrito e contrato faltando seria um documento afirmando
um sha256 que não está em lugar nenhum. Laudo errado é pior que laudo ausente.

## Divisão entre as fases

| Fase | Faz |
|---|---|
| `implement` | Recompõe a classificação, lê o que o contrato declara, grava a proposta e o contrato enriquecido em `evidence/`, monta o pedido de gate. Com pendência não aprovada: grava `state/gate-pendente.json` e devolve **`Blocked`**. |
| `verify` | **Refaz tudo do zero**, confere cobertura, gate e aplicabilidade, roda `lint` sobre a proposta, grava o relatório, **aplica** o enriquecimento no contrato e **emite o laudo**. |

Mesmo desenho de F1, F2 e F3: `verify` não lê a proposta como insumo — o que
mantém `./run.sh verify` válido sozinho e dá de graça a comparação byte a byte
entre as duas fases.

## Regra de PASS

`verify` passa quando **todas**:

1. a classificação recomputada é íntegra pelas regras de F3 —
   `f3_classificar::conferir_cobertura`, chamada, não reescrita;
2. **nenhum item de gate está sem aprovação** — se sobrou algum aqui,
   `implement` deveria ter bloqueado, e o defeito é do harness, não do contrato;
3. toda classificação proposta encontrou a propriedade correspondente no YAML —
   enriquecimento que não achou onde escrever é decisão perdida em silêncio;
4. `datacontract lint` aprova o contrato enriquecido;
5. quando `implement` rodou no mesmo run, a recomputação bate byte a byte com a
   proposta — JSON e YAML.

O item 2 é a asserção do `contexto.md` escrita como teste: *"nunca decide
sozinho o que é persistido"*. O item 4 fecha o critério *"o contrato é válido
contra o padrão ODCS"* sobre a saída, e não só sobre a entrada — que é o único
lugar onde ele ainda podia quebrar.

## Evidência produzida

| Arquivo | Fase | O que é |
|---|---|---|
| `f4-campos-implement.json` / `f4-campos-verify.json` | ambas | saída crua do `export jsonschema`, uma por fase |
| `f4-proposta.json` | `implement` | proposto × declarado por campo, itens de gate e o hash do pedido |
| `f4-contrato-enriquecido.odcs.yaml` | `implement` | o contrato como ficaria — o que `verify` linta e depois aplica |
| `f4-veredito.json` | `verify` | aprovado, defeitos, itens de gate e quais aprovações os cobrem |
| `f4-relatorio.md` | `verify` | o relatório de lacunas que uma pessoa lê sem ferramenta |
| `f4-lint-enriquecido.json` | `verify` | veredito do `lint` sobre a proposta |

Nenhum carrega valor de dado. A trave de privacidade da spec do harness
(seção 6) vale sem exceção.

Fora desta tabela, e de propósito: `contracts/<contrato>/laudos/<versão>-<sha>.md`
— o laudo. Não é evidência de execução, é entregável versionado, e o motivo está
em [O laudo é o entregável](#o-laudo-é-o-entregável-e-não-é-evidência).

## Funções puras e testes

`declaracao_do_yaml`, `itens_de_gate`, `hash_do_gate` e `aplicar` são puras —
recebem YAML como string e devolvem YAML como string. É o que permite exercitar
em `cargo test`, sem container: primeira classificação não abre gate,
despromoção abre, lacuna abre, aprovação com hash diferente não libera,
enriquecimento é idempotente, tag alheia é preservada.

## Onde a implementação mora

`crates/laudo/src/features/f4_gate.rs`, compilada, registrada em `harness::dispatch`. Os
tipos de `state/gate-pendente.json` e `state/aprovacoes.json` moram em
`state.rs`, junto do resto do estado persistido, porque quem os consome é o
comando `approve` — e `main.rs` não pode depender de domínio.
