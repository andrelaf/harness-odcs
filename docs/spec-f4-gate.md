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

## Divisão entre as fases

| Fase | Faz |
|---|---|
| `implement` | Recompõe a classificação, lê o que o contrato declara, grava a proposta e o contrato enriquecido em `evidence/`, monta o pedido de gate. Com pendência não aprovada: grava `state/gate-pendente.json` e devolve **`Blocked`**. |
| `verify` | **Refaz tudo do zero**, confere cobertura, gate e aplicabilidade, roda `lint` sobre a proposta, grava o relatório e **aplica** o enriquecimento no contrato. |

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

## Funções puras e testes

`declaracao_do_yaml`, `itens_de_gate`, `hash_do_gate` e `aplicar` são puras —
recebem YAML como string e devolvem YAML como string. É o que permite exercitar
em `cargo test`, sem container: primeira classificação não abre gate,
despromoção abre, lacuna abre, aprovação com hash diferente não libera,
enriquecimento é idempotente, tag alheia é preservada.

## Onde a implementação mora

`src/features/f4_gate.rs`, compilada, registrada em `features::dispatch`. Os
tipos de `state/gate-pendente.json` e `state/aprovacoes.json` moram em
`state.rs`, junto do resto do estado persistido, porque quem os consome é o
comando `approve` — e `main.rs` não pode depender de domínio.
