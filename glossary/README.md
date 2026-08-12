# glossary/ — o glossário canônico

Um arquivo, `glossario.yaml`, com a lista de termos canônicos da organização.
É a entrada de domínio de [F2 — Mapear](../docs/spec-f2-mapear.md).

## Por que não vive em `contracts/`

`contracts/` tem uma regra escrita: um diretório por contrato, e ali só entra
a fonte escrita à mão daquele contrato ([contracts/README.md](../contracts/README.md)).
O glossário não é um contrato — é o vocabulário contra o qual **todos** os
contratos são lidos. Guardá-lo lá dentro obrigaria a responder "de qual
contrato é esse arquivo?", e a resposta seria "de nenhum".

## Forma de um termo

```yaml
- id: pessoa.cpf                # identidade estável, nunca reaproveitada
  nome: CPF                     # rótulo curto para relatório
  definicao: Numero de ...      # o que o termo significa, em uma frase
  aliases: [cpf, num_cpf]       # como o termo aparece na vida real
```

Os quatro campos são obrigatórios; `aliases` pode ser lista vazia, mas aí o
termo só casa pelo próprio `id`.

## Como o casamento acontece

O nome do campo do contrato é **normalizado** — minúsculas, e qualquer
caractere que não seja letra ou dígito vira `_`, com repetições colapsadas — e
comparado com o conjunto de chaves do glossário. As chaves de um termo são o
`id` normalizado **mais** cada alias normalizado.

Casou: `mapeado`. Não casou: `sem_correspondencia`, que é uma **lacuna** e vai
para o relatório de F4 — não é falha de F2.

**Acento não é normalizado.** `codigo_postal` e `código_postal` são chaves
diferentes, e é para serem: casar por aproximação criaria vínculo que ninguém
declarou, e o projeto inteiro se apoia em decisão declarada e auditável. Grafia
alternativa se resolve acrescentando o alias.

## Integridade — o que torna o glossário inválido

Verificado a cada run, e um FAIL de fase:

- termo sem `id`, sem `nome` ou sem `definicao`;
- dois termos com o mesmo `id`;
- a mesma chave (alias, ou alias colidindo com o `id` de outro termo)
  declarada por mais de um termo — seria ambiguidade, e o harness não escolhe
  sozinho qual dos dois vale;
- arquivo sem `version` ou sem nenhum termo.

Termo que nenhum contrato usa **não** é defeito. O glossário é da organização
e existe antes do contrato que o consome.

## Versão

`version` é semântica e obrigatória, e entra em todo artefato de mapeamento —
`contexto.md` exige registrar qual glossário produziu cada decisão.

| Mudança | Como versionar |
|---|---|
| Acrescentar termo, ou alias a um termo existente | *minor* — o que já casava continua casando |
| Corrigir texto de `nome` ou `definicao` | *patch* |
| Remover ou renomear um `id`, remover alias | *major* — quebra mapeamento já produzido |

Um `id` **nunca** é reaproveitado para outro significado. Mapeamento antigo
guardado em `evidence/` aponta para ele, e reciclar o identificador reescreveria
o passado em silêncio.
