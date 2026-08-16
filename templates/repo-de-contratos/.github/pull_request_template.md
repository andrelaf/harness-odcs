<!--
Branch: <tipo>/<aaaamm>/<descricao-em-kebab-case>, nascida da main.
Rode `./scripts/verificar.sh` antes de abrir — o veredito aqui e o mesmo do CI.
-->

## O que muda no dado

<!-- Campo novo? Campo removido? Mudanca de significado? O diff mostra o YAML;
     escreva aqui o que ele nao mostra. -->

- **Contrato:** `contracts/<dominio>/<contrato>/contract.odcs.yaml`
- **Veredito local** (`./scripts/verificar.sh`): <!-- 0 · 1 · 5 -->

## Para quem aprova

O comentário automático traz o veredito, o resumo por campo e o laudo proposto.
Antes de aprovar:

- [ ] Li o laudo proposto, campo a campo.
- [ ] As lacunas listadas seguem **sem classificação** no contrato, e isso é a decisão pretendida — não um esquecimento.
- [ ] Nenhum campo teve a classificação **rebaixada** sem justificativa escrita acima.

<!-- Lacuna se resolve ampliando o glossário, e o glossário não mora aqui: abra
     a solicitação no repositório do vocabulário e cite este PR. Aceitar que o
     campo siga sem classificação também é uma decisão válida — mas registre-a
     acima, para quem auditar depois. -->

## Mudou `harness.lock`?

<!-- Apague esta seção se não mudou. -->

- [ ] Li as notas do release e sei o que mudou no glossário e no catálogo.
- [ ] Verifiquei o impacto nos contratos existentes — subir de versão pode reclassificar campos sem que nenhum contrato mude.
