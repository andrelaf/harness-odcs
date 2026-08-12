# F2 — Mapear

> Spec curta da feature. O contrato do harness está em
> [`spec-harness.md`](spec-harness.md) e vence em qualquer conflito.

**O que a feature entrega:** cada campo do contrato ODCS sai com uma decisão
registrada — casado com um termo do glossário canônico, ou marcado como lacuna
—, com justificativa por campo e PASS/FAIL explícito sobre a **cobertura**.

O que ela **não** entrega: nada sobre PII, LGPD ou risco (F3), e nenhum
relatório de lacunas para o humano decidir (F4). F2 produz o insumo dos dois;
não os antecipa.

F2 também **não escreve no contrato.** O contrato enriquecido é saída do
projeto, mas persistir enriquecimento é decisão sujeita ao gate humano de F4 —
`contexto.md` é explícito em que o agente "nunca decide sozinho o que é
persistido". Aqui a saída é uma **proposta** em `evidence/`.

---

## Entradas

```
contracts/clientes/contract.odcs.yaml     o contrato (o mesmo alvo de F1)
glossary/glossario.yaml                   o glossário canônico
```

Convenção do glossário, forma de um termo e política de versão:
[`glossary/README.md`](../glossary/README.md).

## Nenhum modelo decide nada aqui

O casamento é determinístico: normaliza o nome do campo e procura a chave no
glossário. Casou ou não casou. Não há chamada de LLM, não há heurística de
similaridade, não há limiar.

Isso é deliberado e é a leitura do `contexto.md`: "as decisões determinísticas
pertencem ao código; o modelo só atua onde há ambiguidade". Em F2 a ambiguidade
não é resolvida — é **nomeada**, vira lacuna, e segue para o humano em F4. Um
casamento por aproximação criaria vínculo que ninguém declarou, exatamente o
tipo de decisão sem rastro que o harness existe para impedir.

## Quem lê o contrato

O `datacontract-cli`, via `export jsonschema`. O harness **não** parseia ODCS.

A alternativa seria ler `schema[].properties[].name` do YAML direto, o que
parece trivial até o primeiro contrato com propriedade aninhada ou com mais de
uma tabela: aí existiriam duas interpretações do padrão no repositório, e a
segunda seria a errada. O motor interpreta; o harness casa e julga.

Consequência assumida: a lista de campos sai em ordem alfabética, e não na
ordem do contrato. O que importa para o artefato é ser **estável entre runs** —
e é, sem depender de nenhuma feature opcional do `serde_json`.

## Divisão entre as fases

| Fase | Faz |
|---|---|
| `implement` | Extrai os campos, carrega o glossário, produz o mapeamento e grava `evidence/<run_id>/f2-mapeamento.json`. |
| `verify` | **Refaz o mapeamento do zero**, confere integridade e cobertura, devolve PASS/FAIL e grava o veredito em `f2-cobertura.json` e a versão legível em `f2-mapeamento.md`. |

`verify` não lê a proposta de `implement` como insumo — recalcula a partir do
contrato e do glossário. É o que faz `./run.sh verify` valer sozinho, como a
spec do harness promete para o comando, e é o mesmo desenho de F1.

### Recalcular tem um efeito de graça

Quando as duas fases rodaram no mesmo run, `verify` compara byte a byte o que
recalculou com o que `implement` gravou. Divergência é FAIL — e só há uma causa
possível: alguma das entradas mudou no meio do run.

Isso é a mesma pergunta que o hash do contrato responde em F1 ("foi o contrato
que mudou ou a ferramenta?"), respondida agora dentro de um único run. Quando
`verify` roda sozinho, não há com o que comparar, e a nota da fase diz isso em
vez de omitir.

A comparação só funciona porque o artefato **não carrega `run_id` nem
timestamp**. É uma restrição de desenho, não um esquecimento: o mesmo contrato
com o mesmo glossário produz o mesmo arquivo em qualquer run, e é isso que
torna dois runs comparáveis com `diff`.

## Regra de PASS

`verify` passa quando **todas**:

1. o glossário é íntegro — as regras estão em
   [`glossary/README.md`](../glossary/README.md#integridade--o-que-torna-o-glossário-inválido);
2. a cobertura é total: cada campo do contrato aparece **exatamente uma vez**
   no mapeamento, e o mapeamento não inventa campo que o contrato não tem;
3. cada decisão é coerente — `mapeado` aponta para um `id` que existe no
   glossário, `sem_correspondencia` não aponta para nenhum, e ambas trazem
   justificativa não vazia;
4. os totais do resumo batem com as decisões;
5. quando `implement` rodou no mesmo run, a recomputação bate com a proposta.

**Campo sem termo não é FAIL.** É lacuna, sai contada e nomeada, e o relatório
para o humano é F4 — está no brief. Cobertura total aqui é cobertura de
**decisão**, não de acerto: o que o harness não pode aceitar é campo que
atravessou o fluxo sem ninguém dizer nada sobre ele. Essa é literalmente a
primeira falha previsível listada em `contexto.md` ("o agente pode esquecer
parte dos campos").

FAIL fica para defeito de verdade: glossário ambíguo, campo decidido duas vezes
ou nenhuma, decisão sem justificativa, contas que não fecham.

### Glossário quebrado para em `implement`

Entrada malformada é falha de **preparação**, não veredito: com um alias
colidindo entre dois termos não existe mapeamento a produzir, porque o harness
não escolhe qual dos dois vale. Então `implement` para ali.

A linha é essa: resultado ruim é julgado em `verify`; **entrada** inutilizável
para antes. É a mesma linha de F1, onde contrato ilegível falha em `implement` e
contrato inválido falha em `verify`. As duas fases chamam a mesma função de
integridade, então não há como divergirem.

## Evidência produzida

| Arquivo | Fase | O que é |
|---|---|---|
| `f2-campos-implement.json` / `f2-campos-verify.json` | ambas | saída crua do `export jsonschema`, uma por fase — os dois arquivos lado a lado são a prova da reprodutibilidade |
| `f2-mapeamento.json` | `implement` | a proposta: decisão, termo, regra e justificativa por campo |
| `f2-cobertura.json` | `verify` | o veredito, com contagens e defeitos encontrados |
| `f2-mapeamento.md` | `verify` | a tabela que uma pessoa lê sem ferramenta |

Nenhum deles carrega valor de dado — o contrato só descreve metadados, e o
mapeamento só cita nome de campo, tipo e identificador de termo. A trave de
privacidade da spec do harness (seção 6) continua valendo sem exceção.

## Funções puras e testes

`normalizar`, `carregar_glossario`, `defeitos_do_glossario`, `ler_campos`,
`mapear` e `conferir_cobertura` são puras: string entra, estrutura sai. Sem
disco, sem container.

É o que permite exercitar glossário ambíguo, cobertura furada e JSON quebrado em
`cargo test`, em vez de precisar montar um contrato defeituoso e subir container
para alcançar cada caminho de falha — que foi o argumento de F1 e vale igual
aqui.

## Onde a implementação mora

`src/features/f2_mapear.rs`, compilada, registrada em `features::dispatch`.
Mesmo motivo de F1: regra de domínio em script seria política fora do binário, e
a segunda IDE passaria a depender do shell certo estar no PATH para o fluxo
decidir a mesma coisa.
