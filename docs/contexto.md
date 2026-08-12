# Contexto do projeto — Mapa de restrições

> Respostas da sondagem inicial (mapa de restrições), que definem o ambiente
> no qual o harness precisa sobreviver. Serve de contexto permanente para o
> desenvolvimento no Claude Code.

---

## Seção 1 — Quem é você e qual responsabilidade técnica carrega?

Arquiteto de Software. Minha responsabilidade técnica é garantir que decisões de arquitetura sejam defensáveis em segurança, privacidade, custo e entrega — e responder por elas, deixando-as rastreáveis. Neste exercício, respondo por garantir que um agente que manipula contratos de dados não classifique, exponha ou altere informação de privacidade sem cobertura de evidência e sem ponto de controle humano.

## Stack e IDE/agente

Rust para a orquestração e as regras (o harness), com o `datacontract-cli` como motor de validação dos contratos. Agente autorizado: Claude Code. As duas IDEs/ambientes são Claude Code e VS Code (execução manual do mesmo ponto de entrada). Repositório e CI no GitHub.

## Domínio

Enriquecimento e classificação de privacidade de contratos de dados no padrão ODCS. Entra um contrato descrevendo um dataset sintético de clientes; saem o mapeamento dos campos para um glossário canônico, a classificação PII/LGPD de cada campo e um relatório de lacunas.

---

## Seção 2 — Ferramentas aprovadas

Rust (toolchain `cargo`), o `datacontract-cli` como motor, um editor de contratos ODCS local, Claude Code e VS Code como ambientes de trabalho, e GitHub para repositório e CI.

## O que é proibido ou inviável

Sem acesso a dados reais ou de produção — apenas datasets sintéticos. Sem rede externa além do necessário. O agente não pode escrever direto na main nem persistir segredos. Nada de processos rodando de forma permanente.

## Limites de custo e tempo

Execução curta e limitada: poucos passos por fluxo e uma janela de tempo definida. Se ultrapassar o limite, o processo é interrompido e escalado para revisão humana.

## Segurança e privacidade

O agente trabalha apenas sobre a estrutura do contrato (metadados), nunca sobre os dados em si. Nenhum valor de campo classificado como PII pode sair. Qualquer reclassificação sensível (de não-PII para PII, ou o contrário) exige aprovação humana antes de valer.

## Auditoria e evidência

Fica registrado tudo o que é necessário para revisão e compliance: o contrato de entrada, cada decisão de classificação com sua justificativa, a versão do glossário usada e a mudança proposta.

---

## Seção 3 — Gatilho e entrada

O fluxo começa quando um contrato ODCS é submetido ou alterado. Entram o próprio contrato, o glossário canônico vigente e o catálogo de classificações de privacidade (LGPD).

## Saída e critério de sucesso

Ao final precisam existir o contrato enriquecido com os campos mapeados, a classificação de privacidade de todos os campos e um relatório de lacunas. Deu certo quando todos os campos estão classificados, nenhum campo PII ficou sem controle e o contrato é válido contra o padrão ODCS.

## Estados e decisões

Validar o contrato, extrair os campos, mapeá-los ao glossário, classificar a privacidade de cada um, apontar lacunas e conflitos, gerar o relatório e, se houver reclassificação sensível, parar para aprovação humana. As decisões determinísticas — validar o formato e reconhecer dados obviamente pessoais, como CPF e e-mail — pertencem ao código; o modelo só atua onde há ambiguidade e nunca decide sozinho o que é persistido.

## Falhas previsíveis

O agente pode esquecer parte dos campos, reprocessar o que já foi feito, encerrar com o relatório incompleto ou classificar algo sem justificativa. Cada um desses pontos tem uma trava: checagem de cobertura total, controle do que já foi processado, exigência de que tudo esteja classificado antes de concluir e justificativa obrigatória por campo.

## Primeiro experimento

Rodar o fluxo sobre um único contrato sintético pequeno, com alguns campos de privacidade óbvios e um ambíguo, sem publicar nada. A hipótese é que o processo cubra todos os campos e encaminhe o caso ambíguo para decisão humana em vez de resolvê-lo sozinho. É reversível porque nada é efetivado — só gera artefatos de teste.

## Frase de síntese

No meu contexto, o harness precisa controlar a cobertura da classificação de privacidade e o que é persistido no contrato — a aprovação humana e o encaminhamento dos casos ambíguos —, porque a restrição de que nenhum campo PII pode ser classificado ou alterado sem evidência e sem controle humano torna arriscado deixar essa decisão apenas no contexto do modelo.
