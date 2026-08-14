<!--
Branch: <tipo>/<aaaamm>/<descricao-em-kebab-case> — verificado no CI.
Ver docs/git-flow.md.

Se este PR nao toca contrato, glossario nem catalogo, apague a segunda secao
inteira. Ela existe para quem vai aprovar o dado, e uma tabela vazia so
atrapalha.
-->

## O que muda, e por quê

<!-- O diff mostra o que mudou. Escreva aqui o que ele nao mostra: por que, e o
     que foi descartado no caminho. -->

## Contrato de dados

<!-- Preencha so se o PR toca contracts/, glossary/ ou classification/. O
     comentario automatico traz o veredito, o resumo por campo e o laudo
     proposto — nao repita nada disso aqui. -->

- **Contrato:** `contracts/<dominio>/<contrato>/contract.odcs.yaml`
- **Veredito local** (`./run.sh check`): <!-- 0 passou · 1 reprovou · 5 aguarda decisao -->

Se o veredito for `5`, quem aprova precisa dizer o que está aprovando:

- [ ] Li o laudo proposto no comentário deste PR, campo a campo.
- [ ] As lacunas listadas seguem **sem classificação** no contrato, e isso é a decisão pretendida — não um esquecimento.
- [ ] Nenhum campo classificado teve o nível **rebaixado** sem justificativa escrita acima.

<!-- Se o PR altera glossary/ ou classification/ junto com um contrato, diga
     aqui por quê. Vale revisão separada: quem escreve o contrato nao deveria
     fechar a propria lacuna no mesmo merge. -->

## Verificação

- [ ] `cargo test` passa
- [ ] `./run.sh check` roda localmente com o mesmo veredito que o CI reportou
- [ ] Docs atualizados, se o comportamento observável mudou
