# Git flow

Uma branch por entrega, um pull request para `main`, e o merge como assinatura.
Não há `develop`, não há `release/`, não há branch de longa duração além de
`main`.

---

## A regra

```
<tipo>/<aaaamm>/<descricao-em-kebab-case>
```

```
feat/202608/verificacao-em-pull-request
feat/202607/contrato-multiplo
fix/202608/laudo-sobrescrito
docs/202608/decisao-de-fechamento
```

| Parte | Valores | Por quê |
|---|---|---|
| `<tipo>` | `feat` · `fix` · `docs` · `chore` | Diz o que a branch é antes de alguém abrir o diff. |
| `<aaaamm>` | ano e mês de **abertura**, 6 dígitos | Ordena a listagem de branches por quando o trabalho começou e torna óbvio o que ficou parado. Não é a data da entrega, e não se atualiza se a branch atravessar o mês. |
| `<descricao>` | minúsculas, dígitos e hífen | O que muda, não onde. `feat/202608/f4-gate` envelhece mal; `feat/202608/gate-humano-em-lacuna` continua legível quando a numeração das features mudar. |

A convenção é verificada no CI — [`workflows/pr-convencao.yml`](../.github/workflows/pr-convencao.yml).
Ela **reprova o PR**, e é o único check deste repositório que reprova por uma
questão de processo em vez de conteúdo. É barato: renomear a branch e empurrar
de novo custa dois comandos.

## O ciclo

```bash
# 1. Sempre da main atualizada. Nunca do topo da branch anterior.
git checkout main && git pull
git checkout -b feat/202608/gate-humano-em-lacuna

# 2. Trabalhe. Antes de abrir o PR, rode o que o CI vai rodar:
./run.sh check                    # 0 passou · 1 reprovou · 5 aguarda decisão

# 3. Abra o PR para main. É ele que inicia o processo.
git push -u origin feat/202608/gate-humano-em-lacuna
gh pr create --base main --fill
```

**O pull request para `main` é o gatilho de tudo.** Ele dispara a verificação do
contrato, monta o comentário com o laudo proposto, e — pelo `CODEOWNERS` — chama
para a revisão quem responde por aquele dado. Nada disso acontece por push
direto, e é por isso que `main` é protegida.

Depois do merge, a branch morre. Ela não é reaproveitada para a entrega
seguinte, e a próxima nasce de `main` de novo.

## Por que da `main`, e não do topo da anterior

Porque já erramos isso aqui, e o erro está registrado em
[`docs/decisao.md`](decisao.md):

> **Branch por feature virou pilha linear.** Cada branch nasceu do topo da
> anterior em vez da `main`, e a `main` nunca recebeu nada. Funciona, mas o
> histórico não mostra quatro entregas independentes — mostra uma só, longa.

Uma pilha linear parece inofensiva enquanto tudo entra. O custo aparece quando
alguma coisa **não** entra: a terceira branch carrega a segunda, que carrega a
primeira, e reverter a do meio deixa de ser uma operação — vira uma
reconstrução. Nascer de `main` é o que mantém cada entrega revertível sozinha.

## Merge

**Merge commit**, não squash e não rebase-and-merge.

O histórico deste repositório é parte do produto: o gate do F4 é liberado por
uma revisão aprovada, e é no histórico que fica registrado quem aprovou o quê,
preso ao sha256 do contrato daquele momento. Squash apaga a fronteira entre a
entrega e os passos dentro dela; rebase reescreve os SHAs que o laudo e as
aprovações referenciam.

```bash
git checkout main
git merge --no-ff feat/202608/gate-humano-em-lacuna
```

`--no-ff` mesmo quando o fast-forward seria possível: sem o commit de merge, a
entrega deixa de existir como unidade e `git log --first-parent` não consegue
mais listar o que entrou em `main`.

## Mensagem de commit

Independente do nome da branch, e com prefixo próprio:

```
feat: o que passou a ser verdade, no imperativo e em uma linha

Por que, e o que foi descartado no caminho. O diff mostra o que mudou; a
mensagem existe para o que ele não mostra.
```

Prefixos em uso: `feat:`, `fix:`, `docs:`, `harness:` (o que a própria máquina
de estados grava no handoff).

## O que ainda não é automático

Dito em voz alta para não parecer esquecimento:

- **Ninguém apaga a branch depois do merge.** Ligue *Automatically delete head
  branches* nas configurações do repositório.
- **A data no nome não é verificada contra a data de criação da branch.** O CI
  confere o formato, não a veracidade. Mentir ali é possível e não faria sentido
  nenhum.
- **`main` protegida é configuração de repositório, não deste arquivo.** Sem os
  itens em [`.github/README.md`](../.github/README.md#2-branch-protection-na-branch-de-destino),
  tudo acima é etiqueta, não regra.
