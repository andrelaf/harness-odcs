# F1 — Validar

> Spec curta da feature. O contrato do harness está em
> [`spec-harness.md`](spec-harness.md) e vence em qualquer conflito.

**O que a feature entrega:** o contrato ODCS do repositório é válido contra o
schema, com PASS/FAIL explícito e evidência que um humano consegue ler.

O que ela **não** entrega: nada sobre glossário, PII ou lacunas. Isso é F2, F3 e
F4.

---

## Alvo

**Resolvido por run, não constante.** Um repositório de dados tem N contratos, e
o harness precisa saber em qual está trabalhando: o caminho vive em
`Run::contrato`, escolhido uma vez na abertura do run e não reconsultado depois
— as fases de domínio precisam concordar sobre qual arquivo estão lendo,
classificando e escrevendo.

```
./run.sh next                                                  # 1 contrato: resolve sozinho
./run.sh next --contrato contracts/clientes/contract.odcs.yaml  # 2 ou mais: obrigatório
```

Sem escolha explícita, `contrato::resolver` só resolve quando não há
ambiguidade. Com dois ou mais contratos ele **recusa e lista** — adivinhar qual
contrato classificar é a decisão errada para tomar em silêncio. Caminho fora de
`contracts/`, ou com `..`, é recusado: o harness escreve neste arquivo, e um
argumento de linha de comando não precisa ter o poder de apontar para qualquer
lugar do disco.

O caminho resolvido é relativo à raiz e usa `/`. O mesmo literal serve ao host e
ao container — a raiz do repositório é montada em `/home/datacontract`, que é o
diretório de trabalho do CLI. Traduzir caminho em dois lugares é onde esse tipo
de código quebra ao mudar de sistema.

## A convenção de nome também é verificada aqui

F1 valida o contrato contra o padrão **e contra a convenção do repositório**.
São perguntas diferentes: o `datacontract lint` responde *"isto é ODCS válido?"*;
a convenção responde *"isto está onde alguém vai conseguir achar?"*. Um contrato
pode passar na primeira e reprovar na segunda — foi o que aconteceu com o
contrato deste repositório, que declarava `id: clientes-sintetico` morando em
`contracts/clientes/`.

As regras estão em [`contracts/README.md`](../contracts/README.md#a-convenção-de-nome-e-por-que-ela-é-verificada);
as funções que as aplicam — `defeitos_do_caminho`, `defeitos_da_identidade` e
`avisos_do_caminho` — são puras e testadas sem container.

Os defeitos de nome são conferidos **antes** da chamada ao container, porque são
baratos, e reportados **junto** com os do lint, e não em vez deles. Quem abriu o
PR precisa ver tudo de uma vez, em lugar de descobrir um problema novo a cada
push. Falta do nível de domínio é **aviso**, não defeito: funciona, mas custa o
roteamento por `CODEOWNERS`.

## Divisão entre as fases

| Fase | Faz |
|---|---|
| `implement` | Lê o contrato e registra a identidade dele (bytes + `sha256`). |
| `verify` | Roda `datacontract lint --output-format json` gravando `evidence/<run_id>/f1-lint.json`, lê o veredito, devolve PASS/FAIL e — só quando passa — gera `evidence/<run_id>/f1-relatorio.html` via `datacontract export html`. |

`implement` **prepara**, `verify` **julga e comprova**. A separação é o que faz
`./run.sh verify` valer sozinho: a fase reexecuta a validação inteira sem
depender de nada que o run anterior tenha deixado em memória — que é o
comportamento que a spec do harness promete para o comando.

O relatório nasce em `verify`, e não em `implement`, por um motivo achado no
uso: `datacontract export html` valida o contrato antes de exportar. Gerar o
relatório antes do lint faria todo contrato inválido morrer em `implement`, com
a mensagem de um exportador em vez do motivo da reprovação — o julgamento
aconteceria na fase errada, anunciado errado. Sobra `implement` fino, e isso é
honesto: F1 é uma feature de verificação, e o trabalho dela **é** o veredito.

Lint aprovando e exportador recusando é FAIL: os dois validam o mesmo contrato,
então a discordância é defeito de ferramenta — e a evidência prometida não
existe.

O hash do contrato em `implement` existe para uma pergunta específica: quando
dois runs discordarem, foi o contrato que mudou ou a ferramenta? Sem o hash, não
há como responder sem adivinhar.

## Regra de PASS

`verify` passa quando, **as três coisas**:

1. o CLI sai com código `0`;
2. o relatório JSON traz `result: passed` **e** nenhum check individual
   reprovado; e
3. o caminho segue a convenção do repositório e o `id` do contrato bate com o
   diretório em que ele mora.

Confiar só no campo agregado deixaria passar relatório internamente
inconsistente. Divergência entre exit code e veredito é reportada como defeito
de integração, não como contrato inválido — são coisas diferentes e um FAIL mudo
esconderia isso.

O motivo da reprovação vem do campo `reason` de cada check e entra na nota da
fase. FAIL sem motivo não é evidência de nada.

`ler_veredito` é função pura e tem teste unitário: valido, inválido,
inconsistente e JSON quebrado. A alternativa seria só conseguir exercitar o
caminho de falha subindo container com contrato quebrado — caro e frágil.

## Onde a implementação mora

`src/features/f1_validar.rs`, compilada, e não em `features/f1-validar/*.sh`.

A spec do harness descreve o primeiro nível da resolução como
`features/<feature-id>/<fase>` — o *slot* por feature, que aqui é a função
`features::f1_validar::<fase>`. Um script no lugar seria o caminho por onde a
regra de domínio vazaria para fora do binário, contra o princípio "política em
Rust, shell é burro": a segunda IDE passaria a depender do shell certo estar no
PATH para o fluxo decidir a mesma coisa.

## Por que a feature foi refeita

F1 foi marcada `done` no run `20260812T033309Z-7dbb68` com `implement` e
`verify` ainda em no-op. O fluxo estava correto; a feature, vazia. O `reset`
existe por causa disso, e o no-op agora anuncia a si mesmo na saída da fase em
vez de passar calado.

O run antigo não foi apagado: comparar os dois é o ponto.
