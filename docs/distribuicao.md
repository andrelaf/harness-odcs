# Distribuição — como o harness sai deste repositório

Um repositório de contratos não tem Rust, não tem `src/` e não deve ter o
vocabulário que julga seus contratos. Este documento descreve o artefato que
torna isso possível, e por que ele tem a forma que tem.

Referência de uso, do outro lado: [`andrelaf/data-contracts`](https://github.com/andrelaf/data-contracts).

---

## As três raízes

Era uma só, e uma só não sobrevive a sair daqui.

| Variável | O quê | Neste repo | Num repo de contratos |
|---|---|---|---|
| `HARNESS_ROOT` | os **dados** — `contracts/`, e onde `trace/` e `evidence/` são escritos | a raiz do repo | o checkout dos contratos |
| `HARNESS_HOME` | a **ferramenta** — binário e scripts | a raiz do repo | o pacote extraído |
| `HARNESS_VOCAB` | o **critério** — `glossary/` e `classification/` | a raiz do repo | o pacote extraído |

Os defaults fazem as três coincidirem (`HARNESS_HOME` cai em `HARNESS_ROOT`,
`HARNESS_VOCAB` cai em `HARNESS_HOME`), então operar daqui não mudou em nada.

No Rust isso custou duas linhas: `cfg.root.join(GLOSSARIO)` virou
`cfg.vocab.join(...)`, e o mesmo para o catálogo. O resto já estava certo desde
o começo — `Config::from_env` sempre leu a raiz de variável de ambiente, sem
nenhum caminho embutido no binário.

**O container nunca vê o vocabulário.** `datacontract_args` monta apenas a raiz
de dados; a classificação é Rust puro lendo YAML, e o motor externo só linta e
exporta o contrato. A separação não vaza para dentro do Docker.

## O pacote

```
harness-odcs/
  harness.sh                 entrypoint de quem usa — não compila nada
  bin/harness-odcs           o binário, em release
  scripts/env.sh             a configuração, fonte única
  scripts/imagem.sh          garante a imagem do motor no digest fixado
  glossary/                  o critério
  classification/            o critério
  VERSION                    procedência: versão, commit, sha256 do vocabulário
```

Montado por [`scripts/package.sh`](../scripts/package.sh), que roda igual na
máquina de quem desenvolve e no job de release. Se o empacotamento morasse no
YAML do workflow, o pacote testado localmente não seria o publicado — e a
diferença só apareceria no repositório de outra pessoa.

**Dois entrypoints, um `env.sh`.** `run.sh` é de quem desenvolve o harness e
compila; `harness.sh` é de quem apenas o usa e não compila. O `env.sh` escolhe o
binário por existência — `bin/` no pacote, `target/debug/` aqui.

A inversão que importa está no `harness.sh`: ele resolve `HARNESS_HOME` pelo
próprio `dirname` e `HARNESS_ROOT` pelo diretório de trabalho. **A ferramenta
visita o repositório**, em vez de o repositório conter a ferramenta.

## Publicar

```bash
git tag v0.1.0 && git push origin v0.1.0
```

[`.github/workflows/release.yml`](../.github/workflows/release.yml) roda os
testes, monta o pacote, gera o tarball e publica o release com o **sha256 nas
notas**. Um release que não passa nos testes é um defeito distribuído por versão
fixada — pior que um defeito local, porque agora ele tem número e alguém vai
fixá-lo de propósito.

## Consumir

No repositório de contratos, um arquivo de pin — versão **e** sha256:

```sh
# harness.lock
HARNESS_REPO=andrelaf/harness-odcs
HARNESS_VERSAO=v0.1.0
HARNESS_SHA256=<das notas do release>
```

Só a versão não bastaria: uma tag pode ser movida, e no dia em que for, o mesmo
contrato passa a ser julgado por outro critério sem que nada tenha mudado no
repositório de contratos. É a mesma disciplina que já aplicamos à imagem do
`datacontract-cli` em `scripts/env.sh`, e pelo mesmo motivo.

O pipeline então faz cinco coisas, e nenhuma delas é decidir:

```bash
gh release download "$HARNESS_VERSAO" --repo "$HARNESS_REPO" …   # baixa
sha256sum --check --strict                                        # confere
tar -xzf …                                                        # extrai
"$PKG/scripts/imagem.sh"                                          # motor, no digest
"$PKG/harness.sh" check --formato github --saida report.json      # julga
```

O mesmo `harness.lock` é lido pelo script local de quem escreve o contrato. Se a
fixação existisse em dois lugares, o CI e a máquina de quem desenvolve
discordariam no dia em que um dos dois subisse de versão.

## O vocabulário vai dentro do pacote — por ora

É uma escolha reversível, não uma convicção.

**O que ela garante:** quem escreve o contrato não tem o arquivo que o julga.
Não por convenção nem por `CODEOWNERS` bem preenchido — por ausência. O gate do
F4 depende disso, e essa é a forma mais barata de torná-lo estrutural.

**O que ela custa:** acoplar duas cadências. O vocabulário muda muito mais rápido
que o código, e hoje adicionar um termo exige publicar uma versão nova do
binário. Um steward de dados não deveria precisar de um release de Rust para
cadastrar `segmento`.

**Como sai disso, quando doer:** `HARNESS_VOCAB` já é a variável. O glossário
ganha repositório próprio, com dono próprio; o pacote deixa de carregá-lo; o
pipeline passa a fixar duas versões em vez de uma, e a apontar `HARNESS_VOCAB`
para o segundo checkout. Nenhuma linha de Rust muda.

**O que não pode acontecer em nenhuma das duas formas:** o vocabulário morar no
repositório de contratos. Aí o autor fecha a própria lacuna, e o gate vira
carimbo.

## Limites, ditos em voz alta

- **Só `linux-x64`.** É o que os runners usam. Quem desenvolve no Windows ou no
  macOS monta o pacote localmente com `./scripts/package.sh` — o script é
  multiplataforma, o release é que não.
- **O pacote não é assinado**, só tem sha256. Contra tag movida, basta; contra
  um release forjado por quem tenha acesso de escrita ao repositório, não.
  Assinatura é a próxima camada, e ela só faz sentido depois de haver mais de um
  consumidor.
- **`VERSION` registra a procedência, mas nada a verifica em tempo de execução.**
  O binário confia no vocabulário que encontra em `HARNESS_VOCAB`. O que amarra
  os dois é o laudo, que grava versão e sha256 do glossário e do catálogo que de
  fato julgaram — a verificação é posterior e documental, não preventiva.
