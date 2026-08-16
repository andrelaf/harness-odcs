# data-contracts

Contratos de dados no padrão **ODCS**, verificados e classificados quanto a
privacidade em cada pull request.

Este repositório guarda **o dado descrito** — e nada mais. A ferramenta que
valida e o vocabulário que classifica vêm de fora, fixados por versão e sha256
em [`harness.lock`](harness.lock).

---

## Por que a ferramenta não mora aqui

Porque quem escreve o contrato não pode escrever o critério que o julga.

O gate de privacidade só significa alguma coisa se o autor de um contrato não
puder, no mesmo pull request, adicionar ao glossário o termo que fecha a própria
lacuna, ou rebaixar no catálogo a classificação do próprio campo. Deixar
glossário e catálogo em outro artefato transforma isso em impossibilidade — não
em política de revisão bem preenchida.

Consequência que vale saber: **subir a versão em `harness.lock` pode mudar a
classificação de um campo sem que nenhum contrato mude.** Por isso é um pull
request, revisado, com dono próprio no `CODEOWNERS`.

## Estrutura

```
contracts/<dominio>/<contrato>/
    contract.odcs.yaml          o contrato — a única coisa que você edita
    laudos/<versao>-<sha>.md    o laudo de classificação, emitido e nunca sobrescrito
harness.lock                    qual ferramenta e qual vocabulário julgam este repo
```

O nível de domínio não é decoração: é ele que permite ao `CODEOWNERS` rotear a
revisão para quem responde por aquele dado.

## Verificar antes de abrir o PR

```bash
./scripts/preparar.sh      # baixa o pacote fixado (idempotente)
./scripts/verificar.sh     # o mesmo veredito que o pull request vai dar
```

**Opcional.** Você não precisa de nada disso para abrir um PR — escreva o YAML e
empurre. Serve só para antecipar o veredito e encurtar o ciclo.

Requisitos: Docker e `gh` autenticado. **Não** requer Rust — o binário vem
pronto no pacote.

O exit code é o veredito:

| | |
|---|---|
| `0` | passou |
| `1` | reprovou — o motivo sai ancorado no arquivo |
| `5` | bloqueado, aguardando decisão humana — não é erro seu |

**Quem emite o laudo é a esteira, não você.** Ao abrir o PR, ela roda a
verificação, emite os documentos e os commita na sua branch — antes da revisão,
para que quem aprova veja de uma vez exatamente o que vai entrar.

Isso é deliberado: o laudo é documento de governança, e o que ele afirma não pode
depender do que estava instalado na máquina de quem escreveu o contrato. Ele
nasce sempre no mesmo ambiente, com a versão fixada em `harness.lock`.

O que chega ao repositório:

| Arquivo | Para quem |
|---|---|
| `contract.odcs.yaml` | ganha `classification` por campo |
| `laudos/<v>-<sha>-<criterio>.md` | quem revisa e quem audita |
| `laudos/<v>-<sha>-<criterio>.html` | quem decide sobre o dado e não lê YAML |
| `laudos/<v>-<sha>-<criterio>.proposta.json` | consulta automatizada |

O nome carrega contrato **e** critério: subir o glossário ou o catálogo emite um
laudo novo ao lado, nunca por cima. Duas constatações sobre o mesmo contrato são
duas constatações, e é exatamente esse par que a auditoria quer comparar.

Se a verificação reprovar (`exit 1`), nada é emitido: não há proposta válida a
registrar. O status fica vermelho e quem abriu o PR corrige com um commit novo.

## O ciclo

```bash
git checkout -b feat/202608/cadastro-clientes
# edite contracts/clientes/cadastro/contract.odcs.yaml
./scripts/verificar.sh
git push -u origin feat/202608/cadastro-clientes
gh pr create --base main --fill
```

O pull request dispara a verificação, comenta o laudo proposto e, pelo
`CODEOWNERS`, chama para revisão quem responde pelo domínio. Quando o veredito é
`5`, o job **não fica vermelho** — nada está errado no contrato, falta decisão
humana. Quem segura o merge é a revisão aprovada.

## O que fica no repositório, e o que não fica

| Fica | Não fica |
|---|---|
| `contracts/` e os laudos emitidos | glossário e catálogo de classificação |
| `harness.lock` — a versão do critério | o binário do validador |
| a configuração do pipeline | `evidence/` e `trace/` — são artefato do job |
