//! `check` — a verificacao efemera, e o que o CI executa.
//!
//! O fluxo (`next`) **escreve**: aplica o enriquecimento no contrato, emite o
//! laudo, grava pedido de gate, commita no handoff. Isso serve a quem esta
//! trabalhando no contrato, e nao serve a um pull request: tres PRs abertos ao
//! mesmo tempo disputariam `state/progress.json`, e cada run de CI viraria um
//! commit conflitando com os outros dois.
//!
//! `check` responde a mesma pergunta sem nenhuma dessas consequencias. Ele
//! calcula tudo — nome, lint, mapeamento, classificacao, gate, contrato
//! enriquecido e laudo — e **nao escreve fora de `evidence/` e `trace/`**. O
//! contrato nao e tocado, `state/` nao e tocado, nada e commitado. A saida e um
//! veredito, um exit code e um `report.json` neutro de plataforma.
//!
//! **O julgamento e o mesmo.** `check` nao reimplementa regra nenhuma: chama
//! `defeitos_do_caminho`, `defeitos_da_identidade`, `ler_veredito`,
//! `compor`, `defeitos_da_composicao` e `lint_do_enriquecido` — as mesmas
//! funcoes que as fases chamam. Se o CI e a maquina de quem desenvolve
//! discordassem sobre o mesmo contrato, o CI deixaria de ser confiavel no dia
//! em que a primeira regra nova entrasse em so um dos dois.
//!
//! **`state/aprovacoes.json` e ignorado aqui, de proposito.** No fluxo local
//! ele libera o gate; num PR ele seria auto-aprovacao — o arquivo esta no
//! repositorio e quem abriu o PR pode commita-lo. Num pull request quem tem
//! autoridade sobre o gate e a revisao de CODEOWNER, e o papel do `check` e
//! **reportar** a pendencia, nunca liberar.

use crate::config::Config;
use crate::ctx::Ctx;
use crate::exit::Exit;
use crate::features::f4_gate::curto;
use crate::features::{contrato, f1_validar, f2_mapear, f3_classificar, f4_gate};
use crate::trace::{self, Draft, Tracer};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

/// O contrato de saida do `check`, estavel e independente de plataforma.
///
/// GitHub e Azure DevOps consomem **este** arquivo; o que muda entre os dois e
/// so o renderizador. Sem ele no meio, cada plataforma leria o texto do console
/// e a portabilidade viraria trabalho de expressao regular.
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Veredito {
    /// Contrato valido, nomeado certo, classificado sem pendencia.
    Pass,
    /// Algo reprovou. O motivo esta em `defeitos`.
    Fail,
    /// Nada errado — falta decisao humana. O motivo esta em `gate`.
    Bloqueado,
}

impl Veredito {
    pub fn exit(self) -> Exit {
        match self {
            Veredito::Pass => Exit::Pass,
            Veredito::Fail => Exit::PhaseFail,
            Veredito::Bloqueado => Exit::Blocked,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Veredito::Pass => "PASS",
            Veredito::Fail => "FAIL",
            Veredito::Bloqueado => "BLOQUEADO",
        }
    }
}

/// Um defeito, com a etapa que o encontrou.
///
/// `etapa` existe para o renderizador agrupar, e `arquivo` para a plataforma
/// prender a anotacao na linha do diff. Motivo que so aparece no log do CI faz
/// quem abriu o PR ter de abrir o log — e ninguem abre.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defeito {
    pub etapa: String,
    pub arquivo: String,
    pub mensagem: String,
}

/// Onde o `check` deixou o que ele **propoe**, sem ter escrito no lugar
/// definitivo. E daqui que o CI monta a sugestao no pull request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Propostas {
    pub contrato: Option<String>,
    pub laudo: Option<String>,
    /// Onde o laudo **iria** morar quando alguem aceitar a proposta.
    pub laudo_destino: Option<String>,
    /// Os anexos do laudo, versionados ao lado dele: a decisao em JSON e a
    /// prova de validade ODCS do contrato commitado.
    #[serde(default)]
    pub anexo_proposta: Option<String>,
    #[serde(default)]
    pub anexo_proposta_destino: Option<String>,
    /// O contrato desenhado em HTML — para quem decide sobre o dado e nao le
    /// YAML. Vai no mesmo commit, entao corresponde ao contrato deste PR.
    #[serde(default)]
    pub anexo_html: Option<String>,
    #[serde(default)]
    pub anexo_html_destino: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relatorio {
    pub schema_version: u32,
    pub run_id: String,
    pub contrato: String,
    pub contrato_sha256: Option<String>,
    pub veredito: Veredito,
    pub exit_code: i32,
    pub glossario_versao: Option<String>,
    pub catalogo_versao: Option<String>,
    pub resumo: Option<f4_gate::ResumoGate>,
    pub defeitos: Vec<Defeito>,
    pub avisos: Vec<String>,
    pub gate_sha256: Option<String>,
    pub gate: Vec<f4_gate::ItemDeGate>,
    /// O vocabulario disponivel, **so quando ha lacuna**.
    ///
    /// Quem escreve contrato descobre a lacuna aqui, no pull request, e ate
    /// entao nomeava campo no escuro: o relatorio dizia que nao casou e nada
    /// sobre o que existia. Isto e a resposta a "como eu deveria ter chamado
    /// este campo?" entregue onde a pergunta aparece.
    ///
    /// Vazio quando nao ha lacuna, e por isso `skip_serializing_if`: o
    /// `report.json` do caso comum nao carrega o vocabulario inteiro.
    ///
    /// **Nao e sugestao.** E a lista do que existe, na ordem do arquivo —
    /// ordenar por semelhanca com o campo que falhou seria palpite, e o binario
    /// promete nunca resolver ambiguidade sozinho. Aproximar e trabalho de
    /// quem le, ou de um plugin que proponha e deixe o `check` julgar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vocabulario: Vec<TermoDisponivel>,
    pub propostas: Propostas,
}

/// Um termo do glossario, reduzido ao que ajuda a nomear um campo.
///
/// `aliases` e a parte util: o `id` diz o que o termo significa, e os aliases
/// dizem como escreve-lo para que o casamento aconteca.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermoDisponivel {
    pub id: String,
    pub nome: String,
    pub aliases: Vec<String>,
}

/// Executa a verificacao inteira e devolve o relatorio.
///
/// Nao recebe `Progress` nem grava um — e agora nem sequer o **carrega**, por
/// nao ter mais como: o que ele monta e um [`Ctx`], que nao tem onde guardar
/// estado de fluxo. Rodar `check` duas vezes em paralelo, em dois pull
/// requests, nao produz disputa por arquivo de estado.
pub fn executar(cfg: &Config, escolha: Option<&str>) -> Result<Relatorio> {
    let alvo = contrato::resolver(&cfg.root, escolha)?;

    let run_id = trace::new_run_id();
    let tracer = Tracer::open(&cfg.trace_dir(), &run_id)?;
    let evidence_dir = cfg.evidence_dir().join(&run_id);
    crate::tools::criar_dir_de_evidencia(&evidence_dir)?;

    // `Ctx`, e nao `Run`: o `check` nunca precisou da maquina de estados.
    //
    // Ate a extracao do `Ctx` ele carregava `FeatureList` e `Progress` do disco
    // so para preencher o struct, com um comentario admitindo que jamais os
    // salvava. Duas leituras de arquivo e um comentario pedindo desculpa, no
    // caminho que o CI executa em todo pull request.
    //
    // `step: 0` porque nao ha passo: `check` nao avanca feature, nao conta
    // passo e nao fecha ciclo. O que correlaciona as invocacoes de ferramenta
    // aqui e o `run_id`.
    let mut ctx = Ctx {
        cfg: cfg.clone(),
        tracer,
        feature_id: "check".to_string(),
        contrato: alvo.clone(),
        evidence_dir,
        tool_seq: 0,
        step: 0,
        notes: Vec::new(),
        riscos: Vec::new(),
    };

    ctx.tracer.emit(
        "run_start",
        Draft {
            feature: Some("check".to_string()),
            msg: format!("check de `{alvo}`"),
            ..Default::default()
        },
    )?;

    let relatorio = montar(&mut ctx, &alvo)?;

    let destino = format!("evidence/{run_id}/report.json");
    let corpo = serde_json::to_string_pretty(&relatorio).context("serializando o relatorio")?;
    fs::write(cfg.root.join(&destino), &corpo).with_context(|| format!("escrevendo {destino}"))?;

    ctx.tracer.emit(
        "run_end",
        Draft {
            feature: Some("check".to_string()),
            result: Some(relatorio.veredito.rotulo().to_string()),
            exit_code: Some(relatorio.veredito.exit().code()),
            msg: format!("relatorio {destino}"),
            ..Default::default()
        },
    )?;

    Ok(relatorio)
}

/// Aplica no repositorio o que o `check` propoe: o contrato enriquecido e o
/// laudo.
///
/// E a contraparte de `check` ser somente-leitura. O CI **propoe** e nunca
/// escreve — nao tem permissao e nao deveria ter, porque o que entra no
/// repositorio precisa passar por revisao. Quem aplica e quem abriu o pull
/// request, na propria maquina, e o resultado vai no diff onde o revisor le.
///
/// Escreve apenas o que ja esta correto: se a verificacao reprovou por defeito
/// real — lint, nome, composicao —, nao ha proposta valida a aplicar, e aplicar
/// mesmo assim gravaria no repositorio um contrato que o proximo `check`
/// recusaria.
///
/// **Gate aberto nao impede.** O laudo registra as lacunas como pendencia, e e
/// exatamente esse documento que o revisor precisa ver no diff antes de
/// aprovar. Esperar o gate fechar para emitir o laudo deixaria o pull request
/// sem o unico artefato que descreve o que esta sendo decidido.
pub fn aplicar(cfg: &Config, escolha: Option<&str>) -> Result<(Relatorio, Vec<String>)> {
    let r = executar(cfg, escolha)?;

    // Defeito que nao seja de aplicacao e impeditivo: o resto do relatorio
    // descreve uma proposta que nasceu de entrada invalida.
    let impeditivos: Vec<&Defeito> = r
        .defeitos
        .iter()
        .filter(|d| d.etapa != "aplicacao")
        .collect();
    if !impeditivos.is_empty() {
        anyhow::bail!(
            "nada a aplicar: a verificacao reprovou em {}",
            impeditivos
                .iter()
                .map(|d| d.etapa.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut escritos = Vec::new();

    let mut mudou_o_contrato = false;
    if let Some(origem) = &r.propostas.contrato {
        let novo = fs::read_to_string(cfg.root.join(origem))
            .with_context(|| format!("lendo a proposta em {origem}"))?;
        let destino = cfg.root.join(&r.contrato);
        let igual = fs::read_to_string(&destino).is_ok_and(|atual| atual == novo);
        if !igual {
            fs::write(&destino, &novo).with_context(|| format!("escrevendo {}", r.contrato))?;
            escritos.push(r.contrato.clone());
            mudou_o_contrato = true;
        }
    }

    // Escrever o contrato muda o que os anexos descrevem.
    //
    // O laudo se refere ao contrato **classificado** e ja nasce estavel; o
    // `.proposta.json` registra o sha do contrato como ele estava ao ser
    // verificado — antes do enriquecimento. Aplicar sem reverificar gravaria
    // um anexo que o proximo `check` acusaria de divergente, e a correcao seria
    // rodar `aplicar` de novo: uma cerimonia que a ferramenta pode poupar.
    //
    // O custo e uma segunda partida de container, e so quando o contrato
    // mudou de fato — que e a primeira aplicacao, nao as seguintes.
    let r = if mudou_o_contrato {
        executar(cfg, escolha)?
    } else {
        r
    };

    if let (Some(origem), Some(destino)) = (&r.propostas.laudo, &r.propostas.laudo_destino) {
        let corpo = fs::read_to_string(cfg.root.join(origem))
            .with_context(|| format!("lendo o laudo em {origem}"))?;
        let path = cfg.root.join(destino);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
        }
        // Laudo emitido nao se sobrescreve. Identico nao e escrita — o
        // documento e deterministico, entao mesmo conteudo no mesmo caminho
        // significa que ele ja existe.
        match fs::read_to_string(&path) {
            Ok(atual) if atual == corpo => {}
            Ok(_) => anyhow::bail!(
                "o laudo `{destino}` ja existe com outro conteudo. \
                 O nome carrega contrato e criterio, entao isto indica algo \
                 fora do lugar — nao sobrescreva sem investigar"
            ),
            Err(_) => {
                fs::write(&path, &corpo).with_context(|| format!("escrevendo {destino}"))?;
                escritos.push(destino.clone());
            }
        }
    }

    // Os anexos, ao contrario do laudo, **sao** reescritos quando divergem: eles
    // nao sao constatacao emitida, sao a mesma constatacao em outro formato. Se
    // o conteudo mudou com o mesmo nome, o laudo ao lado ja teria travado antes.
    for (origem, destino) in [
        (
            &r.propostas.anexo_proposta,
            &r.propostas.anexo_proposta_destino,
        ),
        (&r.propostas.anexo_html, &r.propostas.anexo_html_destino),
    ] {
        let (Some(origem), Some(destino)) = (origem, destino) else {
            continue;
        };
        let corpo = fs::read_to_string(cfg.root.join(origem))
            .with_context(|| format!("lendo o anexo em {origem}"))?;
        let path = cfg.root.join(destino);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
        }
        if !fs::read_to_string(&path).is_ok_and(|atual| atual == corpo) {
            fs::write(&path, &corpo).with_context(|| format!("escrevendo {destino}"))?;
            escritos.push(destino.clone());
        }
    }

    Ok((r, escritos))
}

/// Rele um relatorio ja gravado, para render sem reexecutar.
///
/// E o que separa **executar** de **desenhar**. Sem isto, o CI que quer
/// anotacao, comentario e JSON rodaria o `check` tres vezes — e 99,6% do custo
/// medido deste harness e partida de container, entao tres desenhos custariam
/// tres verificacoes.
pub fn ler(caminho: &std::path::Path) -> Result<Relatorio> {
    let bruto = fs::read_to_string(caminho)
        .with_context(|| format!("lendo o relatorio em {}", caminho.display()))?;
    serde_json::from_str(&bruto)
        .with_context(|| format!("{} nao e um relatorio do `check`", caminho.display()))
}

/// Onde o `report.json` deste run foi gravado.
///
/// Derivado, e nao impresso por `executar`: com `--formato json` o stdout tem
/// de ser **so** JSON, e um `println!` de cortesia no meio do caminho quebraria
/// todo consumidor de maquina — que e exatamente para quem esse formato existe.
pub fn caminho_do_relatorio(r: &Relatorio) -> String {
    format!("evidence/{}/report.json", r.run_id)
}

fn montar(ctx: &mut Ctx, alvo: &str) -> Result<Relatorio> {
    let run_id = ctx.tracer.run_id().to_string();
    let mut defeitos: Vec<Defeito> = Vec::new();

    // --- 1. O nome, antes do container: e barato e nao depende de nada.
    for m in contrato::defeitos_do_caminho(alvo) {
        defeitos.push(Defeito {
            etapa: "nome".to_string(),
            arquivo: alvo.to_string(),
            mensagem: m,
        });
    }
    let avisos = contrato::avisos_do_caminho(alvo);

    let bruto = fs::read_to_string(ctx.cfg.root.join(alvo));
    match &bruto {
        Ok(b) => {
            for m in contrato::defeitos_da_identidade(alvo, b) {
                defeitos.push(Defeito {
                    etapa: "nome".to_string(),
                    arquivo: alvo.to_string(),
                    mensagem: m,
                });
            }
        }
        Err(e) => defeitos.push(Defeito {
            etapa: "nome".to_string(),
            arquivo: alvo.to_string(),
            mensagem: format!("contrato ilegivel ({e})"),
        }),
    }

    // Calculada aqui, e nao em cada saida: as tres reprovacoes abaixo saem por
    // portas diferentes, e uma delas esquecer o carimbo seria o mesmo defeito
    // que este bloco existe para fechar.
    let identidade = identidade_em_vigor(ctx, bruto.as_ref().ok());

    // --- 2. O lint ODCS.
    for m in lint(ctx, alvo, &run_id)? {
        defeitos.push(Defeito {
            etapa: "lint".to_string(),
            arquivo: alvo.to_string(),
            mensagem: m,
        });
    }

    // Contrato invalido nao se classifica: o mapeamento sairia de um documento
    // que o proprio padrao recusa, e todo defeito seguinte seria consequencia
    // deste. Para aqui e reporta o que ja tem.
    if !defeitos.is_empty() {
        return Ok(fechado(run_id, alvo, &identidade, defeitos, avisos));
    }

    // --- 3. Mapeamento, classificacao, gate e enriquecimento.
    let c = match f4_gate::compor(ctx, "check") {
        Ok(c) => c,
        Err(e) => {
            defeitos.push(Defeito {
                etapa: "classificacao".to_string(),
                arquivo: alvo.to_string(),
                mensagem: e,
            });
            return Ok(fechado(run_id, alvo, &identidade, defeitos, avisos));
        }
    };

    for m in f4_gate::defeitos_da_composicao(&c) {
        defeitos.push(Defeito {
            etapa: "classificacao".to_string(),
            arquivo: alvo.to_string(),
            mensagem: m,
        });
    }

    // A proposta vai para `evidence/` — e de la que o CI monta a sugestao. Isto
    // tambem e o que permite lintar o contrato **enriquecido** logo abaixo.
    if let Err(e) = f4_gate::gravar_proposta(ctx, &c) {
        defeitos.push(Defeito {
            etapa: "classificacao".to_string(),
            arquivo: alvo.to_string(),
            mensagem: e,
        });
        return Ok(fechado(run_id, alvo, &identidade, defeitos, avisos));
    }

    match f4_gate::lint_do_enriquecido(ctx) {
        Ok(falhas) => {
            for m in falhas {
                defeitos.push(Defeito {
                    etapa: "lint-enriquecido".to_string(),
                    arquivo: alvo.to_string(),
                    mensagem: m,
                });
            }
        }
        Err(e) => defeitos.push(Defeito {
            etapa: "lint-enriquecido".to_string(),
            arquivo: alvo.to_string(),
            mensagem: e,
        }),
    }

    // --- 4. O laudo proposto, ao lado da proposta de contrato.
    let mut propostas = Propostas {
        contrato: Some(format!(
            "evidence/{run_id}/f4-contrato-enriquecido.odcs.yaml"
        )),
        ..Default::default()
    };
    match laudo_proposto(ctx, &c, &run_id) {
        Ok((onde, destino)) => {
            propostas.laudo = Some(onde);
            propostas.laudo_destino = Some(destino);
        }
        Err(e) => defeitos.push(Defeito {
            etapa: "laudo".to_string(),
            arquivo: alvo.to_string(),
            mensagem: e,
        }),
    }

    // Os anexos do laudo, gravados em `evidence/` como tudo o mais que o
    // `check` produz — e aplicados dali, se alguem aceitar.
    let anexos = Anexos {
        proposta: serde_json::to_string_pretty(&c.proposta).unwrap_or_default(),
        html: html_do_enriquecido(ctx, &run_id, &c.proposta.contrato_sha256),
    };
    if let Some(destino) = &propostas.laudo_destino {
        let (p, h) = Anexos::destinos(destino);
        let base = format!("evidence/{run_id}");
        if fs::write(
            ctx.cfg.root.join(format!("{base}/laudo.proposta.json")),
            &anexos.proposta,
        )
        .is_ok()
        {
            propostas.anexo_proposta = Some(format!("{base}/laudo.proposta.json"));
            propostas.anexo_proposta_destino = Some(p);
        }
        if let Some(corpo) = &anexos.html
            && fs::write(ctx.cfg.root.join(format!("{base}/laudo.html")), corpo).is_ok()
        {
            propostas.anexo_html = Some(format!("{base}/laudo.html"));
            propostas.anexo_html_destino = Some(h);
        }
    }

    // --- 5. A proposta esta aplicada?
    //
    // Sem isto, um pull request podia ser aprovado com o comentario do laudo na
    // tela e o laudo **fora** do repositorio — foi o que aconteceu no primeiro
    // merge real. O comentario e efemero; o que fica e o commit.
    //
    // O laudo tem de ser versionado ao lado do contrato porque e ele que
    // responde "quem classificou este campo assim, e sob qual criterio". Um
    // laudo que so existiu num comentario de PR nao serve a auditoria nenhuma.
    for m in aplicacao_pendente(ctx, &c, &propostas, &anexos) {
        defeitos.push(Defeito {
            etapa: "aplicacao".to_string(),
            arquivo: alvo.to_string(),
            mensagem: m,
        });
    }

    // Gate aberto nao e defeito: nada esta errado, falta decisao humana. Sao
    // exit codes diferentes de proposito — reprovar um PR por lacuna diria a
    // quem o abriu que ele errou, quando o que falta e alguem decidir.
    let veredito = if !defeitos.is_empty() {
        Veredito::Fail
    } else if !c.proposta.gate.is_empty() {
        Veredito::Bloqueado
    } else {
        Veredito::Pass
    };

    Ok(Relatorio {
        schema_version: SCHEMA_VERSION,
        run_id,
        contrato: alvo.to_string(),
        contrato_sha256: Some(c.proposta.contrato_sha256.clone()),
        veredito,
        exit_code: veredito.exit().code(),
        glossario_versao: Some(c.proposta.glossario_versao.clone()),
        catalogo_versao: Some(c.proposta.catalogo_versao.clone()),
        resumo: Some(c.proposta.resumo.clone()),
        defeitos,
        avisos,
        gate_sha256: Some(c.proposta.gate_sha256.clone()),
        gate: c.proposta.gate.clone(),
        vocabulario: vocabulario_se_houver_lacuna(&c),
        propostas,
    })
}

/// O laudo que **seria** emitido, gravado em `evidence/` junto do caminho onde
/// ele iria morar. `check` nao escreve em `contracts/`.
/// Os anexos do laudo: o que fica versionado ao lado dele, para maquina.
///
/// O laudo responde a um humano. Estes dois respondem a uma consulta e a uma
/// auditoria automatizada:
///
///   `.proposta.json`  a decisao inteira — campos, termos, classificacao,
///                     gate, versoes e sha256 dos insumos.
///   `.lint.json`      a prova de que o contrato **commitado** e ODCS valido,
///                     e sob qual versao do motor.
///
/// **Nem toda saida de ferramenta merece ser versionada.** O que sobra em
/// `evidence/<run_id>/` — stdout bruto, lint da fonte antes do enriquecimento,
/// intermediarios — e regeneravel a partir do contrato e do criterio, e
/// commitar tudo faria o repositorio crescer um diretorio por push sem
/// acrescentar nada que o laudo ja nao prove. `evidence/` continua sendo
/// artefato do job, com retencao propria.
///
/// O criterio de corte e **determinismo**: o que entra tem de produzir os
/// mesmos bytes para o mesmo contrato e o mesmo criterio. Sem isso, cada
/// execucao sujaria o diff e a comparacao de conteudo deixaria de funcionar.
struct Anexos {
    proposta: String,
    html: Option<String>,
}

impl Anexos {
    /// `<laudo>.md` -> os anexos, todos com o mesmo nome-base.
    fn destinos(laudo_destino: &str) -> (String, String) {
        let base = laudo_destino.strip_suffix(".md").unwrap_or(laudo_destino);
        (format!("{base}.proposta.json"), format!("{base}.html"))
    }
}

/// O contrato desenhado em HTML, sem a hora em que foi desenhado.
///
/// O `export html` do `datacontract-cli` carimba `Created at <data> UTC` no
/// rodape, e duas execucoes seguidas produzem arquivos diferentes. Versionar
/// assim faria o mesmo contrato gerar um diff por push, e o `check` nao
/// conseguiria exigir o arquivo — a regra que sustenta tudo aqui e comparar
/// conteudo.
///
/// A hora e substituida pela procedencia, que e o que de fato responde "este
/// desenho corresponde a que contrato?". A data de emissao ja e a do commit, e
/// o Git responde por ela melhor que um rodape.
fn html_normalizado(bruto: &str, sha_do_contrato: &str) -> Option<String> {
    // Sem `regex` no projeto — e uma dependencia inteira para uma substituicao.
    // Recorte por delimitadores: o que muda esta entre "Created at " e " with".
    let inicio = bruto.find("Created at ")?;
    let resto = &bruto[inicio..];
    let fim = inicio + resto.find(" with")?;
    Some(format!(
        "{}Gerado do contrato sha256 {}{}",
        &bruto[..inicio],
        &sha_do_contrato[..16.min(sha_do_contrato.len())],
        &bruto[fim..]
    ))
}

/// O que ainda falta estar **no repositorio**, e nao apenas proposto.
///
/// Duas perguntas, e as duas sao de conteudo, nao de existencia: o contrato no
/// disco e igual ao enriquecido? o laudo do destino existe e e igual ao
/// emitido? Comparar conteudo so e possivel porque os dois documentos sao
/// deterministicos — mesmo contrato, mesmo glossario e mesmo catalogo produzem
/// os mesmos bytes, sem data e sem `run_id` dentro.
///
/// O enriquecimento e ponto fixo: aplicar e verificar de novo devolve o mesmo
/// arquivo. Sem isso, exigir a aplicacao criaria uma perseguicao — cada
/// aplicacao mudaria o sha e pediria outra.
/// Desenha o contrato **enriquecido** em HTML, para quem nao le YAML.
///
/// Quem trabalha com produto ou responde pelo dado precisa saber o que ha no
/// dataset sem abrir um `.yaml` de trezentas linhas. O desenho vai ao lado do
/// laudo, no mesmo commit, e por isso corresponde exatamente ao contrato que
/// aquele pull request esta propondo — um HTML publicado em outro lugar
/// envelheceria em silencio.
///
/// Custa uma partida de container a mais por verificacao, e vale porque troca
/// "abra o YAML e interprete" por "abra e leia" para quem decide.
///
/// Falha nao reprova: um desenho ausente e menos grave que uma verificacao
/// interrompida, e o veredito do contrato nao depende dele.
fn html_do_enriquecido(ctx: &mut Ctx, run_id: &str, sha: &str) -> Option<String> {
    let origem = f4_gate::caminho_do_enriquecido(ctx);
    let destino = format!("evidence/{run_id}/f4-contrato.html");
    let saida = ctx
        .datacontract(
            "check-html",
            &["export", "html", &origem, "--output", &destino],
        )
        .ok()?;
    if !saida.ok() {
        return None;
    }
    let bruto = fs::read_to_string(ctx.cfg.root.join(&destino)).ok()?;
    html_normalizado(&bruto, sha)
}

/// O vocabulario disponivel, e **so** quando ha ao menos uma lacuna.
///
/// A condicao nao e economia de bytes: e o que impede o relatorio do caso comum
/// — nenhuma lacuna — de carregar um despejo de catalogo que ninguem pediu. Com
/// 27 termos ja seriam 27 linhas em todo pull request que nao tem pergunta
/// nenhuma a fazer.
///
/// Reclassificacao e conflito nao contam. Nos dois o campo **tem** termo; o que
/// falta e decidir entre duas respostas, e listar o vocabulario nao ajuda em
/// nada a decidir isso.
fn vocabulario_se_houver_lacuna(c: &f4_gate::Composicao) -> Vec<TermoDisponivel> {
    let ha_lacuna = c
        .proposta
        .gate
        .iter()
        .any(|i| i.tipo == f4_gate::TipoDeGate::Lacuna);
    if !ha_lacuna {
        return Vec::new();
    }
    c.glossario
        .termos
        .iter()
        .map(|t| TermoDisponivel {
            id: t.id.clone(),
            nome: t.nome.clone(),
            aliases: t.aliases.clone(),
        })
        .collect()
}

fn aplicacao_pendente(
    ctx: &Ctx,
    c: &f4_gate::Composicao,
    propostas: &Propostas,
    anexos: &Anexos,
) -> Vec<String> {
    let mut faltas = Vec::new();

    match fs::read_to_string(ctx.cfg.root.join(&c.proposta.contrato)) {
        Ok(atual) if atual == c.yaml_enriquecido => {}
        Ok(_) => faltas.push(format!(
            "o contrato nao esta com a classificacao aplicada — \
             `{}` difere do enriquecido que esta verificacao produz. \
             Aplique com `aplicar` e commite",
            c.proposta.contrato
        )),
        Err(e) => faltas.push(format!("contrato ilegivel para comparacao ({e})")),
    }

    // Laudo so e cobrado depois do contrato: com o contrato desatualizado, o
    // destino do laudo e outro, e cobrar os dois produziria duas mensagens para
    // uma causa.
    if !faltas.is_empty() {
        return faltas;
    }

    let Some(destino) = propostas.laudo_destino.as_deref() else {
        return faltas;
    };
    let corpo = propostas
        .laudo
        .as_deref()
        .and_then(|p| fs::read_to_string(ctx.cfg.root.join(p)).ok());
    let Some(corpo) = corpo else {
        return faltas;
    };

    match fs::read_to_string(ctx.cfg.root.join(destino)) {
        Ok(atual) if atual == corpo => {}
        Ok(_) => faltas.push(format!(
            "o laudo `{destino}` existe mas nao corresponde a este criterio. \
             O nome do laudo carrega contrato e criterio, entao isto nao deveria \
             acontecer — nao sobrescreva: investigue"
        )),
        Err(_) => faltas.push(format!(
            "o laudo `{destino}` nao esta no repositorio. \
             Ele e o registro de quem classificou o que, e sob qual criterio — \
             um laudo que so existiu no comentario do pull request nao serve a \
             auditoria. Emita com `aplicar` e commite junto"
        )),
    }

    let (p_dest, h_dest) = Anexos::destinos(destino);
    for (dest, esperado) in [
        (p_dest, Some(&anexos.proposta)),
        (h_dest, anexos.html.as_ref()),
    ] {
        let Some(esperado) = esperado else { continue };
        match fs::read_to_string(ctx.cfg.root.join(&dest)) {
            Ok(atual) if &atual == esperado => {}
            Ok(_) => faltas.push(format!("`{dest}` diverge do que esta verificacao produz")),
            Err(_) => faltas.push(format!(
                "`{dest}` nao esta no repositorio — e o anexo do laudo que \
                 responde a consulta automatizada. Emita com `aplicar`"
            )),
        }
    }
    faltas
}

fn laudo_proposto(
    ctx: &mut Ctx,
    c: &f4_gate::Composicao,
    run_id: &str,
) -> Result<(String, String), String> {
    let versao = f4_gate::versao_do_yaml(&c.yaml_enriquecido).map_err(|e| format!("{e:#}"))?;
    let sha = crate::tools::sha256_hex(&c.yaml_enriquecido);
    let destino = f4_gate::caminho_do_laudo(
        &c.proposta.contrato,
        &versao,
        &sha,
        &f4_gate::sha_do_criterio(&c.laudo),
    );
    let corpo = f4_gate::documento_do_laudo(&c.proposta, &c.laudo, &versao, &sha);

    let onde = format!("evidence/{run_id}/laudo.md");
    fs::write(ctx.cfg.root.join(&onde), &corpo).map_err(|e| format!("escrevendo {onde}: {e}"))?;
    Ok((onde, destino))
}

fn lint(ctx: &mut Ctx, alvo: &str, run_id: &str) -> Result<Vec<String>> {
    let destino = format!("evidence/{run_id}/f1-lint.json");
    let saida = ctx.datacontract(
        "check-lint",
        &[
            "lint",
            alvo,
            "--output-format",
            "json",
            "--output",
            &destino,
        ],
    )?;

    let bruto = match fs::read_to_string(ctx.cfg.root.join(&destino)) {
        Ok(s) => s,
        Err(e) => {
            return Ok(vec![format!(
                "lint saiu com {} e nao deixou relatorio em {destino} ({e})",
                saida.exit_code
            )]);
        }
    };
    let veredito = match f1_validar::ler_veredito(&bruto) {
        Ok(v) => v,
        Err(e) => return Ok(vec![format!("{e:#}")]),
    };

    if veredito.passed && saida.ok() {
        Ok(Vec::new())
    } else if veredito.failures.is_empty() {
        Ok(vec![format!(
            "lint saiu com {} sem check reprovado no relatorio",
            saida.exit_code
        )])
    } else {
        Ok(veredito.failures)
    }
}

/// Quem foi verificado, e sob qual regua — as duas perguntas que uma reprovacao
/// precisa responder sozinha.
///
/// No caminho feliz isto sai da composicao. Quando a verificacao para antes de
/// classificar — nome errado, contrato ilegivel, lint reprovado —, a composicao
/// nunca acontece, e o relatorio saia dizendo apenas "reprovado" com o
/// **caminho** do arquivo. Caminho nao e documento: o mesmo caminho no dia
/// seguinte e outro conteudo, e quem le a reprovacao depois nao tem como saber
/// qual das versoes reprovou nem com qual vocabulario.
///
/// O carimbo diz **em vigor**, e nao **causa**. Defeito de nome e de lint
/// reprovariam sob qualquer catalogo; a etapa de cada defeito e que diz o que
/// falhou. O que a versao responde e outra pergunta, e ela so aparece depois:
/// *"passava na semana passada — mudou o contrato ou mudamos a regua?"*
#[derive(Debug, Default, Clone)]
pub(crate) struct Identidade {
    pub contrato_sha256: Option<String>,
    pub glossario_versao: Option<String>,
    pub catalogo_versao: Option<String>,
}

/// Le glossario e catalogo so para carimbar a versao. Dois `read_to_string` de
/// arquivo que ja esta no disco — barato o bastante para nao valer condicional,
/// e o custo medido do `check` e 99,6% partida de container.
///
/// `None` quando o proprio insumo esta ilegivel, e ai a ausencia e a
/// informacao: um relatorio que nao consegue nomear a regua esta dizendo que a
/// regua e o problema.
fn identidade_em_vigor(ctx: &Ctx, bruto: Option<&String>) -> Identidade {
    Identidade {
        contrato_sha256: bruto.map(|b| crate::tools::sha256_hex(b)),
        glossario_versao: f2_mapear::glossario_do_disco(ctx)
            .ok()
            .map(|(g, _)| g.version),
        catalogo_versao: f3_classificar::catalogo_do_disco(ctx)
            .ok()
            .map(|(c, _)| c.version),
    }
}

/// Relatorio de uma verificacao que parou antes de classificar.
fn fechado(
    run_id: String,
    alvo: &str,
    identidade: &Identidade,
    defeitos: Vec<Defeito>,
    avisos: Vec<String>,
) -> Relatorio {
    Relatorio {
        schema_version: SCHEMA_VERSION,
        run_id,
        contrato: alvo.to_string(),
        contrato_sha256: identidade.contrato_sha256.clone(),
        veredito: Veredito::Fail,
        exit_code: Exit::PhaseFail.code(),
        glossario_versao: identidade.glossario_versao.clone(),
        catalogo_versao: identidade.catalogo_versao.clone(),
        resumo: None,
        defeitos,
        avisos,
        gate_sha256: None,
        gate: Vec::new(),
        // Parou antes de classificar, entao nao ha lacuna a responder: o
        // problema e outro, e listar vocabulario aqui seria ruido no meio de um
        // defeito de schema ou de nome de arquivo.
        vocabulario: Vec::new(),
        propostas: Propostas::default(),
    }
}

// --- Renderizadores ---------------------------------------------------------------
//
// Todos leem o `Relatorio` e nenhum decide nada. E aqui que mora a **unica**
// diferenca entre rodar isto no GitHub e no Azure DevOps: a politica esta em
// `montar`, o veredito esta no exit code, e a plataforma so escolhe como
// desenhar. Portar para outro CI custa uma funcao deste tamanho.

/// Anotacoes do GitHub Actions: aparecem na linha do arquivo, no diff do pull
/// request. Motivo que so existe no log do CI obriga quem abriu o PR a abrir o
/// log — e ninguem abre.
pub fn github(r: &Relatorio) -> String {
    let mut s = String::new();
    for d in &r.defeitos {
        s.push_str(&format!(
            "::error file={},title=contrato: {}::{}\n",
            d.arquivo,
            d.etapa,
            escapar(&d.mensagem)
        ));
    }
    for a in &r.avisos {
        s.push_str(&format!(
            "::warning file={},title=convencao::{}\n",
            r.contrato,
            escapar(a)
        ));
    }
    for i in &r.gate {
        s.push_str(&format!(
            "::warning file={},title=aguarda decisao humana::{}\n",
            r.contrato,
            escapar(&i.linha())
        ));
    }
    s
}

/// Anotacoes do Azure DevOps — irma de `github()`, e a **unica** coisa que muda
/// entre as duas plataformas.
///
/// A politica esta em `montar`, o veredito no exit code e a decisao no
/// `report.json`. O que cada CI sabe fazer e desenhar, e por isso portar custa
/// uma funcao deste tamanho em vez de um projeto.
///
/// `##vso[task.logissue]` e o equivalente de `::error` — prende a mensagem ao
/// arquivo, no lugar de deixa-la so no log do job, que ninguem abre.
pub fn azure(r: &Relatorio) -> String {
    let mut s = String::new();
    for d in &r.defeitos {
        s.push_str(&format!(
            "##vso[task.logissue type=error;sourcepath={}]contrato: {} — {}
",
            escapar_azure_prop(&d.arquivo),
            escapar_azure_msg(&d.etapa),
            escapar_azure_msg(&d.mensagem)
        ));
    }
    for a in &r.avisos {
        s.push_str(&format!(
            "##vso[task.logissue type=warning;sourcepath={}]convencao — {}
",
            escapar_azure_prop(&r.contrato),
            escapar_azure_msg(a)
        ));
    }
    for i in &r.gate {
        s.push_str(&format!(
            "##vso[task.logissue type=warning;sourcepath={}]aguarda decisao humana — {}
",
            escapar_azure_prop(&r.contrato),
            escapar_azure_msg(&i.linha())
        ));
    }
    s
}

/// O escape do Azure DevOps, que **nao** e o do GitHub — e sao **dois**.
///
/// O parser corta a estrutura no primeiro `]` depois de `##vso[`. Entao `;` e
/// `]` so precisam sair das **propriedades**; na mensagem, que vem depois do
/// fecha-colchetes, sao caracteres comuns. Escapar os dois lugares igual produz
/// `[lacuna%5D` no texto que a pessoa le — feio, e por nada.
///
/// `%` usa a sequencia propria `%AZP25`, e nao `%25` como no GitHub.
fn escapar_azure_prop(s: &str) -> String {
    escapar_azure_msg(s).replace(';', "%3B").replace(']', "%5D")
}

fn escapar_azure_msg(s: &str) -> String {
    s.replace('%', "%AZP25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// O corpo do comentario do pull request. Recebe o laudo pronto porque e ele
/// que o revisor tem de ler — deixa-lo so como artefato faria a aprovacao
/// acontecer sem ninguem abrir o documento.
pub fn markdown(r: &Relatorio, laudo: Option<&str>) -> String {
    let mut s = String::new();

    let cabecalho = match r.veredito {
        Veredito::Pass => "### Contrato aprovado na verificacao automatica",
        Veredito::Fail => "### Contrato reprovado na verificacao automatica",
        Veredito::Bloqueado => "### Aguardando decisao humana",
    };
    s.push_str(cabecalho);
    // O sha256 ao lado do caminho, e nao numa linha propria: e a mesma
    // informacao — *qual arquivo* —, e separa-las convidaria a ler so a
    // primeira. Dezesseis digitos e o corte que o resto do produto ja usa.
    s.push_str(&format!("\n\n`{}`", r.contrato));
    if let Some(sha) = &r.contrato_sha256 {
        s.push_str(&format!(" · sha256 `{}`", curto(sha)));
    }
    s.push_str("\n\n");

    if let Some(resumo) = &r.resumo {
        s.push_str(&format!(
            "| Campos | Classificados | Lacunas | Conflitos | Reclassificacoes |\n\
             |---|---|---|---|---|\n| {} | {} | {} | {} | {} |\n\n",
            resumo.campos,
            resumo.classificados,
            resumo.lacunas,
            resumo.conflitos,
            resumo.reclassificacoes
        ));
    }
    // Sai tambem quando reprova, e e ai que ela ganha o sentido: a pergunta
    // que vem depois de "reprovou" nunca e so *o que* — e *mudou o contrato ou
    // mudamos a regua?*. Cada insumo responde por si porque um deles ilegivel e
    // suspeito de ser a causa, e um `if` exigindo os dois esconderia
    // justamente esse caso.
    if r.glossario_versao.is_some() || r.catalogo_versao.is_some() {
        s.push_str(&format!(
            "Glossario {} · catalogo {}\n\n",
            versao_ou_ausente(&r.glossario_versao),
            versao_ou_ausente(&r.catalogo_versao)
        ));
    }

    if !r.defeitos.is_empty() {
        s.push_str("#### Reprovado por\n\n");
        for d in &r.defeitos {
            s.push_str(&format!("- **{}** — {}\n", d.etapa, d.mensagem));
        }
        s.push('\n');
    }

    if !r.gate.is_empty() {
        s.push_str(&format!(
            "#### Pendencias de decisao humana — pedido `{}`\n\n",
            r.gate_sha256
                .as_deref()
                .map(|x| &x[..x.len().min(16)])
                .unwrap_or("-")
        ));
        s.push_str("| Tipo | Campo | Detalhe |\n|---|---|---|\n");
        for i in &r.gate {
            s.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                i.tipo.rotulo(),
                i.campo,
                i.detalhe
            ));
        }
        s.push_str(
            "\nO harness nao libera isto. Destrava com revisao aprovada de CODEOWNER **no \
             SHA atual** — qualquer push novo derruba a aprovacao e o pedido e recalculado.\n\n",
        );

        // O vocabulario, quando ha lacuna. Dentro de `<details>` porque a
        // resposta certa e "disponivel, nao empurrada": quem ja sabe o nome do
        // termo nao precisa rolar por 27 linhas, e quem nao sabe esta a um
        // clique — sem trocar de aba, sem instalar nada.
        if !r.vocabulario.is_empty() {
            s.push_str(&format!(
                "<details>\n<summary><b>Vocabulario disponivel</b> — {} termo(s), glossario v{}</summary>\n\n",
                r.vocabulario.len(),
                r.glossario_versao.as_deref().unwrap_or("?")
            ));
            s.push_str("| Termo | Nome | Como escrever no contrato |\n|---|---|---|\n");
            for t in &r.vocabulario {
                s.push_str(&format!(
                    "| `{}` | {} | {} |\n",
                    t.id,
                    t.nome,
                    if t.aliases.is_empty() {
                        "— (so pelo proprio id)".to_string()
                    } else {
                        t.aliases
                            .iter()
                            .map(|a| format!("`{a}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                ));
            }
            s.push_str(
                "\nA terceira coluna e o que faz o campo casar. Maiuscula, acento, ponto, \
                 hifen e `_` sao ignorados na comparacao — `nome_completo`, `NomeCompleto` e \
                 `nome.completo` sao a mesma chave. O que **nao** casa e nome parecido: \
                 `cpf_cliente` nao encontra `cpf`.\n\n\
                 Nenhum termo serve? Entao a lacuna e real, e se resolve ampliando o \
                 vocabulario — ou aceitando, por decisao registrada, que o campo siga sem \
                 classificacao.\n\n",
            );
            s.push_str("</details>\n\n");
        }
    }

    for a in &r.avisos {
        s.push_str(&format!("> **Aviso** — {a}\n\n"));
    }

    if let (Some(l), Some(d)) = (laudo, &r.propostas.laudo_destino) {
        s.push_str(&format!(
            "<details>\n<summary>Laudo proposto — sera commitado em <code>{d}</code></summary>\n\n{l}\n\n</details>\n"
        ));
    }

    s
}

/// A versao de um insumo do criterio, ou o que dizer quando ela falta.
///
/// Faltar aqui nao e detalhe: os dois insumos estavam no disco quando o `check`
/// rodou, entao ausencia significa **ilegivel** — e um vocabulario ilegivel e o
/// primeiro suspeito da reprovacao que a pessoa esta lendo.
fn versao_ou_ausente(v: &Option<String>) -> String {
    match v {
        Some(v) => format!("v{v}"),
        None => "ilegivel".to_string(),
    }
}

/// `%`, quebra de linha e retorno tem significado nas anotacoes do GitHub e
/// precisam ir codificados, senao a mensagem e truncada na primeira linha.
fn escapar(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// A saida para uma pessoa lendo o terminal.
pub fn imprimir(r: &Relatorio) {
    print!("contrato   {}", r.contrato);
    if let Some(sha) = &r.contrato_sha256 {
        print!("  sha256 {}", curto(sha));
    }
    println!();
    println!("veredito   {}", r.veredito.rotulo());
    // Uma linha, e depois do veredito: quem roda `check` no terminal esta
    // olhando para a linha de cima. Esta so precisa estar no scroll quando a
    // pergunta seguinte aparecer — e ela aparece com "passava ontem".
    if r.glossario_versao.is_some() || r.catalogo_versao.is_some() {
        println!(
            "criterio   glossario {} · catalogo {}",
            versao_ou_ausente(&r.glossario_versao),
            versao_ou_ausente(&r.catalogo_versao)
        );
    }

    if let Some(resumo) = &r.resumo {
        println!(
            "campos     {} — {} classificado(s), {} lacuna(s), {} conflito(s)",
            resumo.campos, resumo.classificados, resumo.lacunas, resumo.conflitos
        );
    }
    for a in &r.avisos {
        println!("  aviso     {a}");
    }
    for d in &r.defeitos {
        println!("  reprovado [{}] {}", d.etapa, d.mensagem);
    }
    if !r.gate.is_empty() {
        println!(
            "  gate      {} item(ns), pedido {}",
            r.gate.len(),
            r.gate_sha256
                .as_deref()
                .map(|s| &s[..s.len().min(16)])
                .unwrap_or("-")
        );
        for i in &r.gate {
            println!("              {}", i.linha());
        }
        println!("            libera com revisao de CODEOWNER no SHA atual");
        // No terminal, o ponteiro e nao a lista: 27 termos empurrariam o
        // veredito para fora da tela, que e a unica linha que quem roda `check`
        // esta olhando. A lista inteira vai no comentario do pull request, onde
        // cabe dentro de um `<details>`, e no `report.json`.
        if !r.vocabulario.is_empty() {
            println!(
                "  vocabulario {} termo(s) no glossario v{} — a lista esta no relatorio \
                 (`report --formato markdown`)",
                r.vocabulario.len(),
                r.glossario_versao.as_deref().unwrap_or("?")
            );
        }
    }
    if let Some(p) = &r.propostas.contrato {
        println!("proposta   {p}");
    }
    if let (Some(l), Some(d)) = (&r.propostas.laudo, &r.propostas.laudo_destino) {
        println!("laudo      {l}  ->  {d}");
    }
    println!("relatorio  {}", caminho_do_relatorio(r));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(veredito: Veredito) -> Relatorio {
        Relatorio {
            schema_version: SCHEMA_VERSION,
            run_id: "20260814T000000Z-aaaaaa".to_string(),
            contrato: "contracts/clientes/contract.odcs.yaml".to_string(),
            contrato_sha256: None,
            veredito,
            exit_code: veredito.exit().code(),
            glossario_versao: None,
            catalogo_versao: None,
            resumo: None,
            defeitos: Vec::new(),
            avisos: Vec::new(),
            gate_sha256: None,
            gate: Vec::new(),
            vocabulario: Vec::new(),
            propostas: Propostas::default(),
        }
    }

    /// Os tres desfechos sao exit codes diferentes de proposito: reprovar um PR
    /// por lacuna diria a quem o abriu que ele errou, quando o que falta e
    /// alguem decidir.
    #[test]
    fn cada_veredito_tem_o_seu_exit_code() {
        assert_eq!(Veredito::Pass.exit().code(), 0);
        assert_eq!(Veredito::Fail.exit().code(), 1);
        assert_eq!(Veredito::Bloqueado.exit().code(), 5);
    }

    #[test]
    fn parada_antes_de_classificar_reprova_e_carrega_o_motivo() {
        let d = vec![Defeito {
            etapa: "nome".to_string(),
            arquivo: "contracts/X/contract.odcs.yaml".to_string(),
            mensagem: "nao e kebab-case".to_string(),
        }];
        let ident = Identidade {
            contrato_sha256: Some("a".repeat(64)),
            glossario_versao: Some("1.2.0".to_string()),
            catalogo_versao: Some("1.1.0".to_string()),
        };
        let r = fechado(
            "run".to_string(),
            "contracts/X/contract.odcs.yaml",
            &ident,
            d,
            Vec::new(),
        );
        assert_eq!(r.veredito, Veredito::Fail);
        assert_eq!(r.exit_code, 1);
        assert!(r.resumo.is_none() && r.propostas.contrato.is_none());
    }

    /// A reprovacao tem de dizer **qual arquivo** e **sob qual regua**. Sem
    /// isso ela aponta para um caminho, e caminho nao e documento: o mesmo
    /// caminho no dia seguinte e outro conteudo.
    #[test]
    fn parada_antes_de_classificar_carimba_arquivo_e_criterio() {
        let ident = Identidade {
            contrato_sha256: Some("b".repeat(64)),
            glossario_versao: Some("1.2.0".to_string()),
            catalogo_versao: Some("1.1.0".to_string()),
        };
        let r = fechado(
            "run".to_string(),
            "contracts/X/contract.odcs.yaml",
            &ident,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(r.contrato_sha256.as_deref(), Some("b".repeat(64).as_str()));
        assert_eq!(r.glossario_versao.as_deref(), Some("1.2.0"));
        assert_eq!(r.catalogo_versao.as_deref(), Some("1.1.0"));

        let md = markdown(&r, None);
        assert!(md.contains("sha256 `bbbbbbbbbbbbbbbb`"), "{md}");
        assert!(md.contains("Glossario v1.2.0 · catalogo v1.1.0"), "{md}");
    }

    /// Insumo ilegivel nao pode sumir atras do que foi lido: se o glossario nao
    /// abre, ele e o primeiro suspeito da reprovacao que a pessoa esta lendo.
    #[test]
    fn insumo_ilegivel_aparece_nomeado_em_vez_de_sumir() {
        let mut r = base(Veredito::Fail);
        r.resumo = None;
        r.glossario_versao = None;
        r.catalogo_versao = Some("1.1.0".to_string());

        let md = markdown(&r, None);
        assert!(md.contains("Glossario ilegivel · catalogo v1.1.0"), "{md}");
    }

    /// Contrato ilegivel nao tem sha — e o relatorio nao pode inventar um nem
    /// entrar em panico tentando encurta-lo.
    #[test]
    fn sem_sha_o_cabecalho_sai_so_com_o_caminho() {
        let ident = Identidade::default();
        let r = fechado(
            "run".to_string(),
            "contracts/X/contract.odcs.yaml",
            &ident,
            Vec::new(),
            Vec::new(),
        );
        let md = markdown(&r, None);
        assert!(md.contains("`contracts/X/contract.odcs.yaml`"), "{md}");
        assert!(!md.contains("sha256"), "{md}");
        assert!(!md.contains("Glossario"), "{md}");
    }

    /// A anotacao tem de prender no arquivo, senao ela cai no log e ninguem le.
    #[test]
    fn anotacao_do_github_leva_arquivo_e_titulo() {
        let mut r = base(Veredito::Fail);
        r.defeitos.push(Defeito {
            etapa: "lint".to_string(),
            arquivo: "contracts/clientes/contract.odcs.yaml".to_string(),
            mensagem: "logicalType invalido".to_string(),
        });
        let s = github(&r);
        assert!(s.starts_with("::error file=contracts/clientes/contract.odcs.yaml,"));
        assert!(s.contains("title=contrato: lint::logicalType invalido"));
    }

    /// O escape do Azure e de **dois** tipos, e trocar um pelo outro quebra em
    /// silencio: `;` na propriedade corta a estrutura no meio; `]` na mensagem,
    /// escapado a toa, aparece como `%5D` para quem le.
    #[test]
    fn azure_escapa_propriedade_e_mensagem_de_formas_diferentes() {
        assert_eq!(escapar_azure_prop("a;b]c"), "a%3Bb%5Dc");
        assert_eq!(escapar_azure_msg("a;b]c"), "a;b]c");
        // `%` tem sequencia propria no Azure — `%25` do GitHub nao serve.
        assert_eq!(escapar_azure_msg("100%"), "100%AZP25");
    }

    /// A anotacao tem de prender no arquivo, como no GitHub — a diferenca esta
    /// so na sintaxe que cada plataforma entende.
    #[test]
    fn azure_prende_a_anotacao_no_arquivo() {
        let mut r = base(Veredito::Fail);
        r.defeitos.push(Defeito {
            etapa: "lint".to_string(),
            arquivo: "contracts/clientes/contract.odcs.yaml".to_string(),
            mensagem: "logicalType invalido".to_string(),
        });
        let s = azure(&r);
        assert!(s.starts_with("##vso[task.logissue type=error;"), "{s}");
        assert!(
            s.contains("sourcepath=contracts/clientes/contract.odcs.yaml]"),
            "{s}"
        );
        assert!(s.contains("logicalType invalido"), "{s}");
    }

    /// Quebra de linha nao codificada trunca a anotacao na primeira linha.
    #[test]
    fn anotacao_codifica_quebra_de_linha_e_porcento() {
        assert_eq!(escapar("a\nb"), "a%0Ab");
        assert_eq!(escapar("100%"), "100%25");
    }

    #[test]
    fn comentario_traz_o_laudo_e_o_destino_dele() {
        let mut r = base(Veredito::Bloqueado);
        r.propostas.laudo_destino = Some("contracts/clientes/laudos/1.0.0-abc1234.md".to_string());
        let s = markdown(&r, Some("# Laudo\n\ncorpo do laudo"));
        assert!(s.contains("Aguardando decisao humana"));
        assert!(s.contains("corpo do laudo"), "{s}");
        assert!(s.contains("contracts/clientes/laudos/1.0.0-abc1234.md"));
    }

    /// Gate no comentario tem de dizer quem libera — senao o revisor procura um
    /// botao no harness que nao existe.
    #[test]
    fn comentario_diz_que_quem_libera_e_o_codeowner() {
        let mut r = base(Veredito::Bloqueado);
        r.gate_sha256 = Some("4f773bab5f6e400f16bd".to_string());
        r.gate.push(crate::features::f4_gate::ItemDeGate {
            tipo: crate::features::f4_gate::TipoDeGate::Lacuna,
            campo: "segmento".to_string(),
            detalhe: "campo sem termo no glossario".to_string(),
        });
        let s = markdown(&r, None);
        assert!(s.contains("CODEOWNER"), "{s}");
        assert!(s.contains("SHA atual"), "{s}");
        assert!(s.contains("4f773bab5f6e400f"), "{s}");
    }

    fn com_lacuna() -> Relatorio {
        let mut r = base(Veredito::Bloqueado);
        r.glossario_versao = Some("1.1.0".to_string());
        r.gate.push(crate::features::f4_gate::ItemDeGate {
            tipo: crate::features::f4_gate::TipoDeGate::Lacuna,
            campo: "sobrenome".to_string(),
            detalhe: "campo sem termo no glossario".to_string(),
        });
        r
    }

    /// Quem tem lacuna precisa saber **o que existe**, e os aliases sao a
    /// resposta pratica: o `id` diz o significado, o alias diz como escrever
    /// para casar.
    #[test]
    fn lacuna_traz_o_vocabulario_e_os_aliases_no_comentario() {
        let mut r = com_lacuna();
        r.vocabulario.push(TermoDisponivel {
            id: "pessoa.nome_completo".to_string(),
            nome: "Nome completo".to_string(),
            aliases: vec!["nome_completo".to_string(), "nome_civil".to_string()],
        });
        let s = markdown(&r, None);
        assert!(s.contains("Vocabulario disponivel"), "{s}");
        assert!(s.contains("glossario v1.1.0"), "{s}");
        assert!(s.contains("`pessoa.nome_completo`"), "{s}");
        assert!(s.contains("`nome_civil`"), "{s}");
    }

    /// Dentro de `<details>`: disponivel, nao empurrado. Quem ja sabe o nome do
    /// termo nao deveria rolar por dezenas de linhas para chegar no resto.
    #[test]
    fn o_vocabulario_vai_recolhido() {
        let mut r = com_lacuna();
        r.vocabulario.push(TermoDisponivel {
            id: "contato.email".to_string(),
            nome: "Endereco de e-mail".to_string(),
            aliases: vec!["email".to_string()],
        });
        let s = markdown(&r, None);
        assert!(s.contains("<details>") && s.contains("</details>"), "{s}");
    }

    /// A condicao que impede o despejo de catalogo no caso comum: sem lacuna, o
    /// comentario nao carrega vocabulario nenhum.
    #[test]
    fn sem_lacuna_o_comentario_nao_carrega_vocabulario() {
        let r = base(Veredito::Pass);
        assert!(r.vocabulario.is_empty());
        let s = markdown(&r, None);
        assert!(!s.contains("Vocabulario disponivel"), "{s}");
    }

    /// Termo sem alias casa so pelo proprio `id`, e a coluna precisa dizer isso
    /// em vez de ficar em branco — celula vazia parece defeito de renderizacao.
    #[test]
    fn termo_sem_alias_diz_que_casa_pelo_id() {
        let mut r = com_lacuna();
        r.vocabulario.push(TermoDisponivel {
            id: "perfil.segmento".to_string(),
            nome: "Segmento comercial".to_string(),
            aliases: Vec::new(),
        });
        let s = markdown(&r, None);
        assert!(s.contains("so pelo proprio id"), "{s}");
    }

    /// O `report.json` do caso comum nao deve engordar com um campo vazio.
    #[test]
    fn vocabulario_vazio_nao_entra_no_json() {
        let r = base(Veredito::Pass);
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("vocabulario"), "{json}");
    }
}
