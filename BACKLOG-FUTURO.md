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

**Renderizador para Azure DevOps.** Um `Formato::Azure` emitindo
`##vso[task.logissue …]`, irmão de `github()`. O `azure-pipelines.yml` já está no
template, mas ainda usa `--formato texto` e nunca rodou num pipeline real.

## Reduz atrito de quem escreve contrato

**`harness termos [--buscar cpf]`** — listar e buscar o vocabulário disponível.
Hoje a pessoa nomeia campo no escuro e só descobre a lacuna quando o pull request
já está aberto. Provavelmente a mais útil das três.

**`harness novo --dominio clientes --contrato pedidos`** — esqueleto no caminho
certo, com `id` batendo com o diretório, já passando na convenção de nome. Hoje as
regras se descobrem errando.

**`harness sugerir`** — para cada campo sem termo, mostrar os termos parecidos.
Transforma "lacuna" em "você quis dizer `contato.telefone`?".

**Plugin de editor — em repositório próprio, e não aqui.** A separação não é de
escopo, é de natureza: este projeto promete determinismo, e um plugin com modelo
dentro não pode prometer isso. Juntá-los abriria a porta para alguém pedir ao
modelo que "ajude a decidir" uma classificação — e a garantia que sustenta o
laudo cairia junto.

Há uma ordem obrigatória: o plugin só existe depois de `termos`. Sem ela, ele
reimplementa o casamento com o glossário, e passam a existir duas
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
