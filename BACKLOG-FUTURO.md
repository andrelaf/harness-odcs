# Backlog futuro

Tudo o que foi deliberadamente deixado de fora. O brief exige este arquivo por
um motivo: **o risco nº 1 do projeto era o classificador crescer e engolir o
tempo do harness**. Ideia que chega vai para cá, não para o código.

Ordenado por consequência, não por esforço.

---

## Corrige um limite conhecido

> A extração recursiva saiu daqui — virou F5, e está em `docs/spec-f5-aninhado.md`.

**Retenção de `evidence/`.** Cresce sem política. Dezenas de arquivos por dezenas
de execuções, e nada expira. Precisa de limpeza antes de virar problema — já
apontado em [`decisao.md`](docs/decisao.md).

**Laudo com aprovação humana registrada no gate.** Hoje o hash do gate cobre o
texto do item; reescrever a frase de um `detalhe` invalida aprovações antigas.
Falha fechada, que é o lado certo de errar, mas incomoda no uso.

## Separa responsabilidades que hoje viajam juntas

**Vocabulário em repositório próprio.** Glossário e catálogo viajam dentro do
pacote, então adicionar um termo exige publicar uma versão nova do binário — um
*data steward* não deveria precisar de um release de Rust para cadastrar
`segmento`. `HARNESS_VOCAB` já é a variável que separa; falta o repositório, o
dono e o segundo pin no pipeline. Ver [`docs/distribuicao.md`](docs/distribuicao.md).

## Reduz atrito de quem escreve contrato — no pull request, que é onde ele está

**O laudo aponta o vocabulário disponível quando houver lacuna.** Hoje o item de
gate diz `campo sem termo no glossario — segue sem classificacao`
(`src/features/f4_gate.rs:1034`): nomeia a ausência e não diz nada sobre o
vocabulário que existe. Quem lê o comentário descobre que errou e continua sem
saber o que estava disponível.

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
comentário. Os três itens abaixo reduzem atrito *antes* do pull request — num
momento que, no fluxo de hoje, não tem ninguém.

**A condição, escrita uma vez:** só valem a pena depois que alguém adotar a
verificação local. E adotar custa mais do que parece — o release publica apenas
`linux-x64` (um job em `.github/workflows/release.yml`), então quem escreve
contrato em Windows ou macOS precisa de WSL, container ou Rust antes de começar.

**`harness termos [--buscar cpf]`** — listar e buscar o vocabulário disponível.
Passa no critério abaixo e continua sendo do binário; o que falta não é lugar, é
usuário. Enquanto o fluxo for só pull request, quem precisa dessa informação a
recebe pelo laudo, acima.

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

Há uma ordem obrigatória, e ela tem um degrau a mais do que parecia: o plugin só
existe depois de `termos`, e `termos` só existe depois de alguém adotar a
verificação local. O plugin precisa do pacote na máquina pelo mesmo motivo que
ele — então herda a condição inteira, e não é a saída para o bloqueio acima. Sem
`termos`, ele reimplementa o casamento com o glossário, e passam a existir duas
implementações da mesma regra. Com ela, o plugin apenas orquestra — entende o
pedido, redige o YAML, consulta o vocabulário, roda `check`, lê a lacuna. Nunca
julga.

## Escala e operação

**Lote ou CLI de vida longa.** 530 ms por invocação de container, ~11 por
execução. Mil contratos são cerca de 1h40 só de partida de Docker. É a única
mudança que o número justifica — otimizar o Rust não paga.

**Publicar o HTML em Pages ou portal.** Hoje o desenho é commitado ao lado do
contrato, o que garante correspondência. Um portal daria URL para quem não usa
git — mas gerado a partir do commit, nunca commitado em duplicidade.

**Assinatura do pacote.** Hoje há sha256, que basta contra tag movida e não
contra release forjado por quem tenha escrita no repositório. Só faz sentido com
mais de um consumidor.

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
