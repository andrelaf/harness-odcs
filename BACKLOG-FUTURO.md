# Backlog futuro

Tudo o que foi deliberadamente deixado de fora. O brief exige este arquivo por
um motivo: **o risco nº 1 do projeto era o classificador crescer e engolir o
tempo do harness**. Ideia que chega vai para cá, não para o código.

Ordenado por consequência, não por esforço.

---

## A linha de corte

O escopo foi fechado em **2026-08-20**. O que entrou, entrou porque tinha
consumidor no fluxo que existe de verdade — e o fluxo que existe de verdade é
**só o pull request**: ninguém roda `preparar.sh` nem `verificar.sh` na própria
máquina. Esse fato, e não uma estimativa de esforço, decidiu cada linha abaixo.

**O que fechou:**

| Entrega | O que resolveu |
|---|---|
| Vocabulário de 8 → 27 termos | as lacunas reais dos dois contratos, e a enumeração inteira do art. 5º, II — `sensivel` deixou de ser caminho sem dado |
| Vocabulário com release e pin próprios | cadastrar um termo não exige mais publicar um binário |
| `Ctx` separado de `Run` | o domínio parou de receber a máquina de estados |
| Workspace `laudo` + `harness` | a fronteira produto/andaime passou a ser recusada pelo compilador, não afirmada em prosa |
| `evidence/` fora do versionamento | este repositório passou a obedecer a regra que ele mesmo escreveu |
| Vocabulário no comentário do PR | quem tem lacuna passou a ver o que existe, com os aliases |
| Carimbo de identidade na reprovação | reprovar deixou de apontar para um caminho — diz o sha256 do arquivo e a versão da régua em vigor |

**O que ficou de fora, e é o resto deste arquivo.** Nada aqui está adiado por
falta de tempo: cada item tem uma condição escrita que ainda não aconteceu, e a
mais frequente delas é *"alguém precisar disso"*.

**Uma coisa não é escopo e continua pendente:** publicar `v0.8.0` e
`vocab-v1.1.0`. O `novo-repo-de-contratos.sh` reprova de propósito até existir
um release `vocab-v*`, e repositório de contratos que já existe precisa do
workflow novo mais as três linhas novas no `harness.lock`, no mesmo commit.

---

## Corrige um limite conhecido

> A extração recursiva saiu daqui — virou F5, e está em `docs/spec-f5-aninhado.md`.

> **Retenção de `evidence/` saiu daqui — resolvida.** E não do jeito que este
> item previa: não faltava política de expiração, faltava este repositório
> obedecer a que já estava escrita. O `artefatos.md` sempre classificou a
> evidência como efêmera e o template do repositório de contratos já a ignorava.
> Uma linha de `.gitignore`, e não um comando `podar` — que teria custado código
> novo para decidir "quantos dias?" no lugar de alguém.

**Laudo com aprovação humana registrada no gate.** O hash do gate cobre o texto
do item; reescrever a frase de um `detalhe` invalida aprovações antigas. Falha
fechada, que é o lado certo de errar.

Ficou de fora pelo mesmo critério que derrubou `termos`: **só incomoda num fluxo
que ninguém usa.** O `check` ignora `state/aprovacoes.json` de propósito — num
pull request ele seria auto-aprovação, e quem tem autoridade sobre o gate é a
revisão de CODEOWNER (`crates/laudo/src/check.rs:22-26`). O arquivo tem uma
entrada, de quando o próprio harness foi construído. Enquanto o fluxo for só
pull request, o incômodo não tem quem o sinta.

## Separa responsabilidades que hoje viajam juntas

**Vocabulário em repositório próprio — falta a mudança de casa, e só ela.**

A parte cara está feita: glossário e catálogo saíram do pacote do binário e
ganharam versão, release (`vocab-v*`) e pin próprios no `harness.lock`.
Cadastrar `segmento` já não exige publicar Rust, e o `CODEOWNERS` já dá revisor
distinto para `/glossary/` e `/classification/`. Ver
[`docs/distribuicao.md`](docs/distribuicao.md).

O que falta não é código: **um dono do vocabulário que não seja o dono do
binário.** Sem ele, um repositório separado só muda o endereço do problema — a
mesma pessoa continua aprovando o contrato e o critério que o julga.

No dia em que existir, muda **uma linha**: `HARNESS_VOCAB_REPO`. Os dois
diretórios trocam de casa e nada mais no pipeline se move.

## Reduz atrito de quem escreve contrato — no pull request, que é onde ele está

> **Entregue.** O comentário do pull request passou a trazer o vocabulário
> disponível — `id`, nome e **aliases** — quando há lacuna, dentro de um
> `<details>` para ficar disponível sem ser empurrado. No terminal fica o
> ponteiro; a lista inteira vai no comentário e no `report.json`.
>
> A dose acabou sendo a lista, e não o ponteiro que eu previa aqui: os aliases
> são a resposta prática a *"como eu deveria ter chamado este campo?"*, e um
> ponteiro para o glossário não os entrega. A condição `só quando há lacuna` é
> o que impede o despejo de catálogo no caso comum.

**O texto abaixo é o registro do problema, de antes da entrega.** O item de gate
dizia `campo sem termo no glossario — segue sem classificacao`
(`crates/laudo/src/features/f4_gate.rs:1034`): nomeava a ausência e não dizia nada
sobre o vocabulário que existe. Quem lia o comentário descobria que errou e
continuava sem saber o que estava disponível.

É o mesmo dado que o `termos` imprimiria, entregue onde a pessoa já está lendo —
sem instalação, sem pacote novo por plataforma, sem depender do plugin. E não
fere o `docs/decisao.md`: **listar** o vocabulário é fato. Ordenar por
similaridade continua sendo `sugerir`, e continua fora.

Há uma dose a escolher: apontar para o glossário na versão fixada do
`harness.lock` (barato, sempre certo) ou embutir a lista dos termos no comentário
(mais útil, ruidoso quando o glossário crescer). Começar pelo ponteiro.

**É o único item desta seção com consumidor.** Os outros três estão abaixo.

## Bloqueado — espera um loop local que ninguém adotou

Esta seção assumia que existe alguém com o pacote na máquina. **Não existe.** O
uso real do repositório de contratos é só via pull request: ninguém roda
`scripts/preparar.sh` nem `scripts/verificar.sh`, e o veredito chega pelo
comentário. Os três primeiros itens abaixo reduzem atrito *antes* do pull
request — num momento que, no fluxo de hoje, não tem ninguém.

**A condição, escrita uma vez:** só valem a pena depois que alguém adotar a
verificação local. E adotar custa mais do que parece — o release publica apenas
`linux-x64` (um job em `.github/workflows/release.yml`), então quem escreve
contrato em Windows ou macOS precisa de WSL, container ou Rust antes de começar.

> **O plugin, no fim da seção, não está mais preso a essa condição.** Ele
> precisa do vocabulário, e não do binário — e o vocabulário virou YAML com
> release próprio, que baixa em qualquer sistema. O que o segura é outra coisa:
> repositório próprio, e alguém que o queira.

**`harness termos [--buscar cpf]`** — listar e buscar o vocabulário disponível.
Passa no critério abaixo e continua sendo do binário; o que falta não é lugar, é
usuário. Enquanto o fluxo for só pull request, quem precisa dessa informação a
recebe pelo comentário, acima — com `id`, nome e aliases.

O que sobra dele, se alguém adotar a verificação local, é **buscar**: consultar
antes de escrever, em vez de descobrir depois de abrir o pull request. É menos
do que este item prometia quando foi escrito.

**`harness novo --dominio clientes --contrato pedidos`** — esqueleto no caminho
certo, com `id` batendo com o diretório. Além de bloqueado, está no lugar errado:
não responde pergunta nenhuma, produz um arquivo. Dois geradores de esqueleto
diferentes nunca *discordam*, porque quem julga o resultado é o `check` depois.
Falha no critério, e é do plugin — ao lado do `sugerir`.

**`sugerir` — no plugin, não no binário.** Mostrar os termos parecidos para um
campo sem correspondência é útil, e **não** deve morar aqui: aproximação é
palpite, e o binário tem a promessa de nunca resolver ambiguidade sozinho.
Heurística de similaridade dentro dele contradiz o `docs/decisao.md` inteiro.

No plugin é honesto — sugestão de assistente, que a pessoa aceita ou descarta, e
que o `check` valida depois. Ele consome `harness termos --json` e faz a parte de
linguagem em cima disso, sem reimplementar o casamento.

**O teste que decide onde uma capacidade mora:** se duas implementações pudessem
discordar sobre a mesma pergunta, ela é do binário. `termos` responde "este nome
casa?" — a mesma pergunta do `check`, e admite uma resposta só. `sugerir`
responde "o que você provavelmente quis dizer?" — pergunta que o `check` nunca
faz. `novo` não responde pergunta: entrega arquivo, e o `check` julga depois.

**Plugin de editor — em repositório próprio, e não aqui.** A separação não é de
escopo, é de natureza: este projeto promete determinismo, e um plugin com modelo
dentro não pode prometer isso. Juntá-los abriria a porta para alguém pedir ao
modelo que "ajude a decidir" uma classificação — e a garantia que sustenta o
laudo cairia junto.

**`termos` deixou de ser pré-requisito dele.** Este arquivo afirmava que o
plugin só existe depois de `termos`, porque sem ele reimplementaria o casamento
e passariam a existir duas implementações da mesma regra. A regra que dispensa
isso é mais simples: **o plugin propõe, o `check` julga.**

Sugestão errada custa uma ida e volta; veredito errado custa a confiança nos
dois. Enquanto o plugin nunca disser "passou", ele pode ler o YAML do
vocabulário e aproximar do jeito que quiser — aproximar já é palpite por
natureza, e o `check` adjudica depois de qualquer forma. O que ele não pode é
pronunciar veredito, e isso não depende de `termos` existir.

O que o plugin precisa é do **vocabulário na máquina**, e essa parte ficou mais
fácil: ele tem release próprio, é YAML e não é preso a plataforma como o
binário — o mesmo tarball serve em Windows e macOS, sem Docker e sem Rust.

Continua valendo o que o plugin faz: entende o pedido, redige o YAML, consulta o
vocabulário, roda `check`, lê a lacuna. **Nunca julga.**

## Escala e operação

**Lote ou CLI de vida longa.** 530 ms por invocação de container, ~11 por
execução. Mil contratos são cerca de 1h40 só de partida de Docker. É a única
mudança que o número justifica — otimizar o Rust não paga.

**Publicar o HTML em Pages ou portal.** Hoje o desenho é commitado ao lado do
contrato, o que garante correspondência. Um portal daria URL para quem não usa
git — mas gerado a partir do commit, nunca commitado em duplicidade.

**Assinatura dos pacotes.** Hoje há sha256, que basta contra tag movida e não
contra release forjado por quem tenha escrita no repositório. Só faz sentido com
mais de um consumidor.

Agora são **dois** pacotes, e a prioridade entre eles não é óbvia: o do binário
é maior e mais visível, mas quem carrega o critério é o do vocabulário — um
catálogo forjado reclassifica campo em todo contrato que o use, sem que nenhuma
linha de contrato mude. Se um dia só um for assinado, é esse.

**Durabilidade da reprovação no pull request.** O comentário é editado no
lugar (`gh pr comment --edit-last`), então cada push sobrescreve o anterior: no
push que conserta, o "reprovado" vira "aprovado" e o evento desaparece. Somado
ao anexo de 30 dias e à `evidence/` ignorada, a reprovação é efêmera por três
mecanismos independentes.

**Está certo enquanto o pull request está aberto** — o comentário é status vivo,
não log, e postar um comentário novo por push transformaria um PR de oito
pushes em oito comentários de ruído. E o GitHub já guarda o que importa por
SHA: a conclusão do check run é permanente e o resumo do job carrega o corpo
inteiro sem o teto de 60000 caracteres.

A condição para mexer: alguém precisar reconstruir *a sequência* de reprovações
de um PR — auditoria de processo, não de contrato. Se acontecer, o botão é a
retenção do run, não um arquivo na árvore. Laudo é registro de aprovação; um
laudo de reprovação seria nomeado pelo sha256 de um contrato que nunca entra na
`main`, e a esteira teria de apagá-lo no push seguinte para não deixar
documento falso no repositório de quem escreve contrato.

**Agregado de reprovação no release do vocabulário.** O `release-vocabulario.yml`
já roda `check` em todos os contratos deste repositório antes de publicar. O que
ele não faz é dizer **quais mudaram de veredito** — e essa é a única pergunta de
reprovação que sobrevive ao pull request: *"o que a versão nova da régua
quebrou?"*.

Num repositório de contratos isso deixa de ser hipótese: subir
`HARNESS_VOCAB_VERSAO` faz o alvo virar todos os contratos de uma vez, e a
reprovação passa a ser sobre o critério, não sobre o contrato. O lugar do
registro é as notas do release, que já carregam versão e sha256 — não o
repositório de contratos, onde ele seria ruído em N pull requests.

Depende de um baseline: comparar veredito exige guardar o anterior. Hoje o
workflow só sabe o resultado da execução atual.

**Trava para o bit de execução dos scripts.** O `.gitattributes` avisa em prosa
que script novo em `scripts/` precisa de `git update-index --chmod=+x`, porque
no Windows `core.fileMode` é `false` e o `chmod` local não vira commit. O aviso
não funcionou: `package-vocabulario.sh` nasceu com `100644` **depois** dele, e
derrubou o primeiro `vocab-v1.1.0` no passo que monta o pacote.

Nota em comentário não é trava — quem escreve o script seguinte não está lendo
o `.gitattributes`. A verificação cabe numa linha e roda em qualquer
plataforma:

```sh
git ls-files -s '*.sh' | grep 100644 && { echo "script sem bit de execucao"; exit 1; }
```

O lugar dela é o workflow `convencao`, que já é o único que reprova por
processo e já roda em segundos sem container. Ficou de fora do commit que
consertou o modo porque ali o release estava parado, e misturar a trava com o
desbloqueio faria os dois esperarem a mesma revisão.

## Descartado, e por quê

**Laudo em PDF como registro.** A autoridade do laudo vem de estar no git, preso
a um commit e a uma revisão aprovada. PDF não é diffável, embute data de criação
e exige mais uma ferramenta na esteira. Se compliance exigir arquivo, gerar a
partir do Markdown com o sha256 no rodapé — a embalagem, não o registro.

**Excel versionado.** Binário, muda a cada execução a ponto de o tamanho variar,
vira blob reescrito inteiro sem diff legível. Gerar sob demanda.

**LLM classificando campo.** Foi a decisão de projeto e continua sendo: é de onde
vêm as garantias de cobertura verificável e justificativa por decisão. Um modelo
classificaria 200 campos em segundos e perderia exatamente isso.
