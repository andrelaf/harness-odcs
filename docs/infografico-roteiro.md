# Roteiro para infográfico — Harness ODCS

> Texto pronto para colar no NotebookLM (ou em qualquer gerador de imagem).
> Descreve **o que desenhar**, não apenas o que o sistema faz. Os rótulos entre
> aspas devem aparecer literalmente na arte.

---

## Instrução de uma linha

Gere um infográfico horizontal, em três faixas empilhadas, mostrando como um contrato de dados sai da mão de uma pessoa e chega classificado, com evidência auditável, sem que nenhuma decisão sensível seja tomada sem aprovação humana.

## Título e subtítulo

- **Título:** "Da modelagem ao contrato classificado"
- **Subtítulo:** "Harness determinístico para classificação de privacidade em contratos ODCS"

---

## Faixa 1 — Modelagem (topo)

Fluxo da esquerda para a direita, três elementos ligados por seta:

1. Ícone de pessoa, rótulo **"Dev / Data Owner"**
2. Janela de navegador, rótulo **"Editor ODCS"**, e abaixo, em fonte menor: *"container, acessível por link"*
3. Documento YAML, rótulo **"contrato.odcs.yaml"**

Seta saindo do documento para baixo, com o rótulo **"git commit"**. Essa seta é a fronteira do desenho: acima dela é trabalho humano, abaixo é máquina. Marque isso com uma linha horizontal tracejada e o texto **"a partir daqui, nada é manual"**.

## Faixa 2 — O harness (centro, elemento dominante)

O núcleo da imagem. Nove blocos em sequência horizontal, ligados por setas, todos do mesmo tamanho — a uniformidade é proposital, comunica que a ordem é fixa:

**"start" → "plan" → "bearings" → "smoke" → "pick" → "implement" → "verify" → "handoff" → "stop"**

Acima da sequência, uma chave abrangendo todos os blocos com o rótulo **"teto de passos: 12"**.

Abaixo da sequência, três setas de saída divergentes, cada uma em cor distinta:

| Saída | Rótulo | Sublegenda |
|---|---|---|
| verde | **"PASS · exit 0"** | *"feature concluída, commit criado"* |
| vermelha | **"FAIL · exit 1"** | *"para na fase que falhou, não avança"* |
| âmbar | **"AGUARDA HUMANO · exit 5"** | *"reclassificação sensível não passa sozinha"* |

Destaque visual na saída âmbar — é o ponto de controle que justifica o projeto inteiro.

## Faixa 3 — Evidência (base)

Três cartões lado a lado, ligados à faixa 2 por setas pontilhadas verticais:

1. **"state/"** — *"lista de features e cursor do fluxo"*
2. **"trace/"** — *"cada transição, com duração e exit code"*
3. **"evidence/"** — *"saída bruta das ferramentas"*

Sob os três cartões, uma tarja com o texto:
**"O trace guarda nome de campo, decisão e justificativa. Nunca o valor do dado."**

---

## Bloco lateral — as quatro features

Coluna estreita à direita, numerada, com um ícone pequeno cada:

1. **"F1 · Validar"** — contrato válido contra o padrão ODCS
2. **"F2 · Mapear"** — cada campo casado com o glossário canônico
3. **"F3 · Classificar"** — PII/LGPD por campo, com justificativa obrigatória
4. **"F4 · Gate"** — lacunas apontadas, reclassificação sensível pausada

Legenda da coluna: *"uma feature por vez, do início ao fim, antes da próxima"*.

---

## Rodapé

Três números grandes, lado a lado, com legenda curta embaixo:

- **9** — *"fases, sempre na mesma ordem"*
- **12** — *"teto de passos; estourou, aborta"*
- **0** — *"campos PII classificados sem evidência"*

---

## Direção de arte

- Diagrama técnico limpo, não ilustração conceitual. Sem metáforas de robô, cérebro ou engrenagem.
- Paleta sóbria de três cores + neutros. Cor reservada para **estado** (verde/vermelho/âmbar), nunca para decoração.
- Tipografia sem serifa. Rótulos de fase em fonte monoespaçada — são identificadores, não prosa.
- Fundo claro e uniforme. Sem gradiente, sem sombra pesada, sem efeito 3D.
- Densidade: a faixa 2 ocupa cerca de metade da altura total. As faixas 1 e 3 são suporte.
- Formato: horizontal 16:9, legível em projeção.

## O que não desenhar

- Nada de nuvem, servidor ou banco de dados: o harness não tem infraestrutura, e desenhar uma sugere algo falso.
- Nada de setas de retorno ou laços entre fases. O fluxo não volta — é a característica central e desenhar um ciclo a contradiz.
- Nada de rostos, dados pessoais de exemplo, CPF ou e-mail fictício na arte. O projeto trata só de metadados; mostrar valor de campo contradiz a própria mensagem.
