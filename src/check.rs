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
use crate::exit::Exit;
use crate::features::{contrato, f1_validar, f4_gate};
use crate::phases::Run;
use crate::state::{FeatureList, Progress};
use crate::trace::{self, Draft, Tracer};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

/// O contrato de saida do `check`, estavel e independente de plataforma.
///
/// GitHub e Azure DevOps consomem **este** arquivo; o que muda entre os dois e
/// so o renderizador. Sem ele no meio, cada plataforma leria o texto do console
/// e a portabilidade viraria trabalho de expressao regular.
pub const SCHEMA_VERSION: u32 = 1;

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
    pub propostas: Propostas,
}

/// Executa a verificacao inteira e devolve o relatorio.
///
/// Nao recebe `Progress` nem grava um: o estado e carregado so porque `Run`
/// exige, e nunca persistido. Rodar `check` duas vezes em paralelo, em dois
/// pull requests, nao produz disputa por arquivo de estado.
pub fn executar(cfg: &Config, escolha: Option<&str>) -> Result<Relatorio> {
    let alvo = contrato::resolver(&cfg.root, escolha)?;

    let run_id = trace::new_run_id();
    let tracer = Tracer::open(&cfg.trace_dir(), &run_id)?;
    let evidence_dir = cfg.evidence_dir().join(&run_id);
    fs::create_dir_all(&evidence_dir)
        .with_context(|| format!("criando {}", evidence_dir.display()))?;

    let mut run = Run {
        cfg: cfg.clone(),
        // Carregado para satisfazer `Run`, jamais salvo. `check` nao avanca
        // feature, nao conta passo e nao fecha ciclo.
        features: FeatureList::load_or_seed(&cfg.feature_list_path())?,
        progress: Progress::load_or_default(&cfg.progress_path())?,
        tracer,
        feature_id: "check".to_string(),
        contrato: alvo.clone(),
        evidence_dir,
        tool_seq: 0,
        notes: Vec::new(),
        resultados: Vec::new(),
        riscos: Vec::new(),
    };

    run.tracer.emit(
        "run_start",
        Draft {
            feature: Some("check".to_string()),
            msg: format!("check de `{alvo}`"),
            ..Default::default()
        },
    )?;

    let relatorio = montar(&mut run, &alvo)?;

    let destino = format!("evidence/{run_id}/report.json");
    let corpo = serde_json::to_string_pretty(&relatorio).context("serializando o relatorio")?;
    fs::write(cfg.root.join(&destino), &corpo).with_context(|| format!("escrevendo {destino}"))?;

    run.tracer.emit(
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

fn montar(run: &mut Run, alvo: &str) -> Result<Relatorio> {
    let run_id = run.tracer.run_id().to_string();
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

    let bruto = fs::read_to_string(run.cfg.root.join(alvo));
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

    // --- 2. O lint ODCS.
    for m in lint(run, alvo, &run_id)? {
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
        return Ok(fechado(run_id, alvo, defeitos, avisos));
    }

    // --- 3. Mapeamento, classificacao, gate e enriquecimento.
    let c = match f4_gate::compor(run, "check") {
        Ok(c) => c,
        Err(e) => {
            defeitos.push(Defeito {
                etapa: "classificacao".to_string(),
                arquivo: alvo.to_string(),
                mensagem: e,
            });
            return Ok(fechado(run_id, alvo, defeitos, avisos));
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
    if let Err(e) = f4_gate::gravar_proposta(run, &c) {
        defeitos.push(Defeito {
            etapa: "classificacao".to_string(),
            arquivo: alvo.to_string(),
            mensagem: e,
        });
        return Ok(fechado(run_id, alvo, defeitos, avisos));
    }

    match f4_gate::lint_do_enriquecido(run) {
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
    match laudo_proposto(run, &c, &run_id) {
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
        propostas,
    })
}

/// O laudo que **seria** emitido, gravado em `evidence/` junto do caminho onde
/// ele iria morar. `check` nao escreve em `contracts/`.
fn laudo_proposto(
    run: &mut Run,
    c: &f4_gate::Composicao,
    run_id: &str,
) -> Result<(String, String), String> {
    let versao = f4_gate::versao_do_yaml(&c.yaml_enriquecido).map_err(|e| format!("{e:#}"))?;
    let sha = crate::tools::sha256_hex(&c.yaml_enriquecido);
    let destino = f4_gate::caminho_do_laudo(&c.proposta.contrato, &versao, &sha);
    let corpo = f4_gate::documento_do_laudo(&c.proposta, &c.laudo, &versao, &sha);

    let onde = format!("evidence/{run_id}/laudo.md");
    fs::write(run.cfg.root.join(&onde), &corpo).map_err(|e| format!("escrevendo {onde}: {e}"))?;
    Ok((onde, destino))
}

fn lint(run: &mut Run, alvo: &str, run_id: &str) -> Result<Vec<String>> {
    let destino = format!("evidence/{run_id}/f1-lint.json");
    let saida = run.datacontract(
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

    let bruto = match fs::read_to_string(run.cfg.root.join(&destino)) {
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

/// Relatorio de uma verificacao que parou antes de classificar.
fn fechado(run_id: String, alvo: &str, defeitos: Vec<Defeito>, avisos: Vec<String>) -> Relatorio {
    Relatorio {
        schema_version: SCHEMA_VERSION,
        run_id,
        contrato: alvo.to_string(),
        contrato_sha256: None,
        veredito: Veredito::Fail,
        exit_code: Exit::PhaseFail.code(),
        glossario_versao: None,
        catalogo_versao: None,
        resumo: None,
        defeitos,
        avisos,
        gate_sha256: None,
        gate: Vec::new(),
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
    s.push_str(&format!("\n\n`{}`\n\n", r.contrato));

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
    if let (Some(g), Some(c)) = (&r.glossario_versao, &r.catalogo_versao) {
        s.push_str(&format!("Glossario v{g} · catalogo v{c}\n\n"));
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

/// `%`, quebra de linha e retorno tem significado nas anotacoes do GitHub e
/// precisam ir codificados, senao a mensagem e truncada na primeira linha.
fn escapar(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// A saida para uma pessoa lendo o terminal.
pub fn imprimir(r: &Relatorio) {
    println!("contrato   {}", r.contrato);
    println!("veredito   {}", r.veredito.rotulo());

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
        let r = fechado(
            "run".to_string(),
            "contracts/X/contract.odcs.yaml",
            d,
            Vec::new(),
        );
        assert_eq!(r.veredito, Veredito::Fail);
        assert_eq!(r.exit_code, 1);
        assert!(r.resumo.is_none() && r.propostas.contrato.is_none());
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
}
