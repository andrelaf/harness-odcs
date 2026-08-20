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

São **dois**, e a separação entre eles é a mesma das três raízes: a ferramenta e
o critério não mudam pelo mesmo motivo nem na mesma velocidade.

```
harness-odcs/                          harness-vocab/
  harness.sh      entrypoint             glossary/         o que cada termo significa
  bin/harness-odcs  o binário            classification/   o que a lei diz sobre ele
  scripts/env.sh    a configuração       VERSION           versão e sha256 de cada um
  scripts/imagem.sh o motor, no digest
  VERSION           versão, commit
```

Montados por [`scripts/package.sh`](../scripts/package.sh) e
[`scripts/package-vocabulario.sh`](../scripts/package-vocabulario.sh), que rodam
igual na máquina de quem desenvolve e no job de release. Se o empacotamento
morasse no YAML do workflow, o pacote testado localmente não seria o publicado —
e a diferença só apareceria no repositório de outra pessoa.

**Por que o `VERSION` do vocabulário lista os dois arquivos separadamente:**
porque são dois donos com cadências diferentes. O glossário é do *data steward*;
o catálogo é do encarregado de dados. Uma revisão jurídica que corrige uma
justificativa move um e não o outro, e *"qual catálogo classificou este campo?"*
precisa de resposta própria.

**Por que o `VERSION` do binário não fala mais do vocabulário:** ele afirmaria
sobre um conteúdo que não carrega e não controla. O sha256 do glossário saiu
daqui e foi para o pacote que de fato o entrega.

**Dois entrypoints, um `env.sh`.** `run.sh` é de quem desenvolve o harness e
compila; `harness.sh` é de quem apenas o usa e não compila. O `env.sh` escolhe o
binário por existência — `bin/` no pacote, `target/debug/` aqui.

A inversão que importa está no `harness.sh`: ele resolve `HARNESS_HOME` pelo
próprio `dirname` e `HARNESS_ROOT` pelo diretório de trabalho. **A ferramenta
visita o repositório**, em vez de o repositório conter a ferramenta.

## Publicar

Dois fluxos, duas tags, no mesmo repositório:

```bash
git tag v0.8.0       && git push origin v0.8.0         # a ferramenta
git tag vocab-v1.1.0 && git push origin vocab-v1.1.0   # o critério
```

[`release.yml`](../.github/workflows/release.yml) roda os testes, monta o pacote
e publica com o **sha256 nas notas**. Um release que não passa nos testes é um
defeito distribuído por versão fixada — pior que um defeito local, porque agora
ele tem número e alguém vai fixá-lo de propósito.

[`release-vocabulario.yml`](../.github/workflows/release-vocabulario.yml) faz o
mesmo pelo critério, e o guarda dele é o `check` rodando contra os contratos
deste repositório: alias declarado por dois termos, termo do glossário sem
entrada no catálogo e incoerência entre `pii`, `sensivel` e `classification`
reprovam ali, antes de o número existir.

> **`v[0-9]*`, e não `v*`.** Os dois fluxos dividem o espaço de tags, e
> `vocab-v1.1.0` casa com `v*`. Sem o filtro, cadastrar um termo republicaria o
> binário — com uma tag que não bate com o `Cargo.toml`, o que faria a trava de
> versão reprovar o job. Falha barulhenta, por um motivo que ninguém adivinharia.

## Consumir

No repositório de contratos, um arquivo de pin — versão **e** sha256, duas vezes:

```sh
# harness.lock
HARNESS_REPO=andrelaf/harness-odcs
HARNESS_VERSAO=v0.8.0
HARNESS_SHA256=<das notas do release>

HARNESS_VOCAB_REPO=andrelaf/harness-odcs
HARNESS_VOCAB_VERSAO=vocab-v1.1.0
HARNESS_VOCAB_SHA256=<das notas do release>
```

São dois pins porque são duas perguntas de auditoria: *que código julgou?* e
*sob qual vocabulário?*. E é o segundo bloco que responde a pergunta prática de
governança — **subir o vocabulário é um pull request de duas linhas**, revisável,
datado, e que roda o `check` de todos os contratos antes de alguém aprovar.
Enquanto ele não acontece, nenhum contrato muda de veredito.

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

## O vocabulário saiu do pacote

Ele vinha dentro, e o custo era acoplar duas cadências: cadastrar `segmento`
exigia compilar Rust e publicar um binário. Um *data steward* não deveria
depender da fila do dono do código para acrescentar um termo.

**O que a saída não custou:** nenhuma linha de Rust. `HARNESS_VOCAB` sempre foi a
variável — `Config::from_env` nunca teve caminho embutido —, e a prova é
verificável: o pacote com o vocabulário externo emitiu **o mesmo laudo**, com o
mesmo `sha_do_criterio`, que a execução com o vocabulário ao lado do código.
Caminho de entrega diferente, critério idêntico.

**O que ela ganhou de brinde:** o binário é preso a plataforma (`linux-x64`); o
vocabulário é YAML e não é. O segundo pin funciona em Windows e macOS, onde o
primeiro não serve.

**O que substituiu a garantia por ausência.** Enquanto o vocabulário morava no
pacote, quem escrevia o contrato não tinha o arquivo que o julga — por ausência,
que é mais barato que convenção. Agora a garantia é o
[`CODEOWNERS`](../.github/CODEOWNERS) deste repositório, com dono por caminho em
`/glossary/` e `/classification/`, separados entre *data steward* e encarregado
de dados. É uma garantia mais fraca que ausência e mais forte que nada — e é a
que existe sem repositório novo.

**O que continua não podendo acontecer:** o vocabulário morar no repositório de
contratos. Aí o autor amplia o glossário e fecha a própria lacuna no mesmo pull
request, e o gate vira carimbo. É o mesmo buraco de auto-aprovação que faz o
`check` ignorar `state/aprovacoes.json` de propósito, um nível abaixo.

**O que falta para a separação completa:** um dono do vocabulário que não seja o
dono do binário. No dia em que existir, muda **uma linha** —
`HARNESS_VOCAB_REPO` — e os dois diretórios trocam de casa. A costura, que é a
parte cara, já está feita.

## Limites, ditos em voz alta

- **Só `linux-x64`, e só o binário.** É o que os runners usam. Quem desenvolve no
  Windows ou no macOS monta o pacote localmente com `./scripts/package.sh` — o
  script é multiplataforma, o release é que não. O pacote do vocabulário não tem
  esse limite: é YAML, e o mesmo tarball serve em qualquer sistema.
- **O pacote não é assinado**, só tem sha256. Contra tag movida, basta; contra
  um release forjado por quem tenha acesso de escrita ao repositório, não.
  Assinatura é a próxima camada, e ela só faz sentido depois de haver mais de um
  consumidor.
- **`VERSION` registra a procedência, mas nada a verifica em tempo de execução.**
  O binário confia no vocabulário que encontra em `HARNESS_VOCAB`. O que amarra
  os dois é o laudo, que grava versão e sha256 do glossário e do catálogo que de
  fato julgaram — a verificação é posterior e documental, não preventiva. O que o
  pipeline verifica antes é outra coisa: que o **tarball** baixado é o fixado.
- **Nada impede combinar versões que nunca foram testadas juntas.** Os dois pins
  são independentes por desenho, e é isso que permite subir um sem o outro. O
  preço é que `HARNESS_VERSAO=v0.8.0` com `HARNESS_VOCAB_VERSAO=vocab-v9.0.0` é
  uma combinação que ninguém exerceu. Na prática o `check` do pull request roda
  exatamente a combinação fixada antes de alguém aprovar, então a combinação
  nova é exercitada no ato de adotá-la — mas o repositório do harness não tem
  como recusá-la de antemão.
