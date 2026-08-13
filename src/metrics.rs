//! Medicao: custo, duracao, erros e resultado por execucao.
//!
//! **E uma derivacao do trace, nao uma segunda instrumentacao.** Nenhum
//! contador novo e escrito durante a execucao: `duration_ms`, `exit_code` e
//! `result` estao no trace desde a Semana 1 justamente para que a metrica
//! nascesse daqui. Duas instrumentacoes paralelas divergem — e quando
//! divergirem, ninguem sabe qual das duas mente.
//!
//! Consequencia pratica: `./run.sh metrics` regenera o arquivo inteiro a partir
//! de `trace/`. Nao ha estado incremental para corromper, e apagar
//! `metrics/metrics.jsonl` nao perde nada.
//!
//! ## O que "custo" significa aqui
//!
//! Nao ha custo de token: nenhum modelo roda dentro do fluxo — a classificacao
//! e consulta a catalogo, deterministica. O custo real e **tempo de maquina**,
//! e ele se concentra nas invocacoes de processo externo, quase todas
//! `docker run` do `datacontract-cli`. Por isso a metrica separa a duracao
//! total da duracao gasta em ferramenta: e a fracao entre as duas que responde
//! "onde saiu caro".

use crate::trace::Event;
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Uma linha de `metrics/metrics.jsonl` — um run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetricaDeRun {
    pub run_id: String,
    pub feature: Option<String>,
    pub inicio: String,
    pub fim: String,
    /// `PASS`, `FAIL`, `HALT` ou `INCOMPLETO` — este ultimo quando o run nao
    /// chegou a emitir `run_end`, isto e, foi interrompido por fora.
    pub resultado: String,
    pub fases: usize,
    pub passos: u32,
    /// Soma das fases. Nao e relogio de parede: o que fica de fora e a
    /// escrita de estado e de trace entre uma fase e outra, sub-milissegundo.
    pub duracao_ms: u128,
    pub ferramentas: usize,
    pub duracao_ferramentas_ms: u128,
    pub fase_mais_cara: Option<String>,
    pub fase_mais_cara_ms: u128,
    /// Fases que reprovaram. E o numero de erros no sentido do brief: o que
    /// para o fluxo.
    pub erros: usize,
    /// Invocacoes externas com exit code diferente de zero.
    ///
    /// **Nao e o mesmo que erro.** O fluxo sonda o repositorio com
    /// `git rev-parse --verify` para saber se a branch existe, e o exit 1 dessa
    /// sondagem e a resposta "nao existe", nao uma falha. Somar as duas coisas
    /// inflaria a contagem de erros com o funcionamento normal.
    pub ferramentas_nao_zero: usize,
    pub bloqueios: usize,
    pub abortos: usize,
    /// A mensagem do `abort` ou do `blocked`, quando houve. E a resposta a
    /// "onde travou" sem precisar abrir o trace.
    pub motivo_da_parada: Option<String>,
}

impl MetricaDeRun {
    /// Quanto do tempo foi gasto esperando processo externo. `None` quando o
    /// run nao teve duracao medida.
    pub fn fracao_em_ferramentas(&self) -> Option<f64> {
        if self.duracao_ms == 0 {
            return None;
        }
        Some(self.duracao_ferramentas_ms as f64 / self.duracao_ms as f64)
    }
}

/// Funcao pura: o JSONL de um run vira a metrica dele.
///
/// Recebe texto e nao caminho — e o que permite exercitar run abortado, run
/// bloqueado e trace truncado em `cargo test`, sem precisar produzir cada um
/// deles de verdade.
pub fn derivar(run_id: &str, jsonl: &str) -> Result<MetricaDeRun> {
    let mut m = MetricaDeRun {
        run_id: run_id.to_string(),
        feature: None,
        inicio: String::new(),
        fim: String::new(),
        // Sem `run_end` o run nao terminou por conta propria. Dizer
        // `INCOMPLETO` e diferente de assumir falha: interrompido por fora nao
        // e o mesmo que reprovado, e a contagem de erros ficaria errada.
        resultado: "INCOMPLETO".to_string(),
        fases: 0,
        passos: 0,
        duracao_ms: 0,
        ferramentas: 0,
        duracao_ferramentas_ms: 0,
        fase_mais_cara: None,
        fase_mais_cara_ms: 0,
        erros: 0,
        ferramentas_nao_zero: 0,
        bloqueios: 0,
        abortos: 0,
        motivo_da_parada: None,
    };

    let mut vistos = 0usize;
    for (n, linha) in jsonl.lines().enumerate() {
        let linha = linha.trim();
        if linha.is_empty() {
            continue;
        }
        // Uma linha quebrada no fim do arquivo e o caso normal de um processo
        // morto no meio da escrita — o trace e append-only e sem transacao.
        // Descartar so a ultima linha preserva a medicao do que ja estava
        // gravado; descartar o arquivo inteiro perderia o run por causa do
        // byte final.
        let ev: Event = match serde_json::from_str(linha) {
            Ok(e) => e,
            Err(e) if n + 1 == jsonl.lines().count() => {
                m.motivo_da_parada = Some(format!("trace truncado na ultima linha ({e})"));
                break;
            }
            Err(e) => return Err(e).with_context(|| format!("linha {} de {run_id}", n + 1)),
        };

        vistos += 1;
        if m.inicio.is_empty() {
            m.inicio = ev.ts.clone();
        }
        m.fim = ev.ts.clone();
        if m.feature.is_none() {
            m.feature = ev.feature.clone();
        }
        m.passos = m.passos.max(ev.step);

        match ev.event.as_str() {
            "phase_end" => {
                m.fases += 1;
                let d = ev.duration_ms.unwrap_or(0);
                m.duracao_ms += d;
                if d > m.fase_mais_cara_ms {
                    m.fase_mais_cara_ms = d;
                    m.fase_mais_cara = ev.from.clone();
                }
                if ev.result.as_deref() == Some("FAIL") {
                    m.erros += 1;
                }
            }
            "tool_exec" => {
                m.ferramentas += 1;
                m.duracao_ferramentas_ms += ev.duration_ms.unwrap_or(0);
                if ev.exit_code.unwrap_or(0) != 0 {
                    m.ferramentas_nao_zero += 1;
                }
            }
            "blocked" => {
                m.bloqueios += 1;
                m.motivo_da_parada = Some(ev.msg.clone());
            }
            "abort" => {
                m.abortos += 1;
                m.motivo_da_parada = Some(ev.msg.clone());
            }
            "run_end" => {
                if let Some(r) = &ev.result {
                    m.resultado = r.clone();
                }
            }
            _ => {}
        }
    }

    if vistos == 0 {
        anyhow::bail!("trace de {run_id} nao tem nenhum evento legivel");
    }
    Ok(m)
}

/// A leitura do conjunto. E aqui que mora a resposta a "onde travou, onde saiu
/// caro" — as duas perguntas que o brief cobra da medicao.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resumo {
    pub runs: usize,
    pub por_resultado: BTreeMap<String, usize>,
    pub duracao_ms: u128,
    pub duracao_ferramentas_ms: u128,
    pub ferramentas: usize,
    pub erros: usize,
    pub bloqueios: usize,
    pub abortos: usize,
    /// Tempo total por fase, somado entre runs. A fase mais cara do projeto
    /// inteiro sai daqui.
    pub por_fase_ms: BTreeMap<String, u128>,
}

pub fn resumir(metricas: &[MetricaDeRun]) -> Resumo {
    let mut r = Resumo {
        runs: metricas.len(),
        por_resultado: BTreeMap::new(),
        duracao_ms: 0,
        duracao_ferramentas_ms: 0,
        ferramentas: 0,
        erros: 0,
        bloqueios: 0,
        abortos: 0,
        por_fase_ms: BTreeMap::new(),
    };
    for m in metricas {
        *r.por_resultado.entry(m.resultado.clone()).or_insert(0) += 1;
        r.duracao_ms += m.duracao_ms;
        r.duracao_ferramentas_ms += m.duracao_ferramentas_ms;
        r.ferramentas += m.ferramentas;
        r.erros += m.erros;
        r.bloqueios += m.bloqueios;
        r.abortos += m.abortos;
        if let Some(f) = &m.fase_mais_cara {
            *r.por_fase_ms.entry(f.clone()).or_insert(0) += m.fase_mais_cara_ms;
        }
    }
    r
}

impl Resumo {
    pub fn fracao_em_ferramentas(&self) -> Option<f64> {
        if self.duracao_ms == 0 {
            return None;
        }
        Some(self.duracao_ferramentas_ms as f64 / self.duracao_ms as f64)
    }

    /// A fase que mais consumiu tempo somando todos os runs.
    pub fn fase_mais_cara(&self) -> Option<(&str, u128)> {
        self.por_fase_ms
            .iter()
            .max_by_key(|(_, ms)| **ms)
            .map(|(f, ms)| (f.as_str(), *ms))
    }
}

/// Le `trace/*.jsonl` e devolve uma metrica por run, em ordem de `run_id` — que
/// e cronologica por construcao, porque o id comeca pelo timestamp UTC.
pub fn coletar(trace_dir: &Path) -> Result<Vec<MetricaDeRun>> {
    let mut arquivos: Vec<_> = fs::read_dir(trace_dir)
        .with_context(|| format!("lendo {}", trace_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    arquivos.sort();

    let mut out = Vec::new();
    for path in arquivos {
        let run_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let bruto =
            fs::read_to_string(&path).with_context(|| format!("lendo {}", path.display()))?;
        out.push(derivar(&run_id, &bruto)?);
    }
    Ok(out)
}

/// Grava `metrics/metrics.jsonl`, uma linha por run.
///
/// JSONL pelo mesmo motivo do trace: legivel sem ferramenta externa, e uma
/// linha por registro sobrevive a leitura parcial.
pub fn gravar(metricas: &[MetricaDeRun], destino: &Path) -> Result<()> {
    if let Some(dir) = destino.parent() {
        fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    }
    let mut corpo = String::new();
    for m in metricas {
        corpo.push_str(&serde_json::to_string(m).context("serializando a metrica")?);
        corpo.push('\n');
    }
    fs::write(destino, corpo).with_context(|| format!("escrevendo {}", destino.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um run completo e curto, no formato exato que `Tracer::emit` escreve.
    const RUN_OK: &str = r#"
{"ts":"2026-08-13T03:00:00Z","run_id":"r1","seq":1,"feature":"f4-gate","to":"start","event":"run_start","step":0,"msg":"teto 12 passos"}
{"ts":"2026-08-13T03:00:00Z","run_id":"r1","seq":2,"feature":"f4-gate","from":"start","event":"phase_end","result":"PASS","duration_ms":5,"step":1,"msg":""}
{"ts":"2026-08-13T03:00:01Z","run_id":"r1","seq":3,"feature":"f4-gate","result":"PASS","duration_ms":900,"exit_code":0,"event":"tool_exec","step":2,"msg":"docker run ..."}
{"ts":"2026-08-13T03:00:02Z","run_id":"r1","seq":4,"feature":"f4-gate","from":"smoke","event":"phase_end","result":"PASS","duration_ms":1000,"step":2,"msg":""}
{"ts":"2026-08-13T03:00:02Z","run_id":"r1","seq":5,"feature":"f4-gate","result":"PASS","step":2,"event":"run_end","msg":""}
"#;

    #[test]
    fn run_completo_vira_uma_metrica() {
        let m = derivar("r1", RUN_OK).unwrap();
        assert_eq!(m.resultado, "PASS");
        assert_eq!(m.feature.as_deref(), Some("f4-gate"));
        assert_eq!(m.fases, 2);
        assert_eq!(m.passos, 2);
        assert_eq!(m.duracao_ms, 1005);
        assert_eq!(m.ferramentas, 1);
        assert_eq!(m.duracao_ferramentas_ms, 900);
        assert_eq!(m.fase_mais_cara.as_deref(), Some("smoke"));
        assert_eq!(m.erros, 0);
        assert_eq!(m.inicio, "2026-08-13T03:00:00Z");
        assert_eq!(m.fim, "2026-08-13T03:00:02Z");
    }

    /// A fracao que responde "onde saiu caro": quase tudo e espera de
    /// container.
    #[test]
    fn a_fracao_em_ferramentas_e_medida_e_nao_estimada() {
        let m = derivar("r1", RUN_OK).unwrap();
        let f = m.fracao_em_ferramentas().unwrap();
        assert!((0.89..0.90).contains(&f), "{f}");
    }

    /// Exit code diferente de zero numa sondagem nao e erro. O fluxo pergunta
    /// ao git se a branch existe, e "nao existe" volta como exit 1.
    #[test]
    fn ferramenta_com_exit_nao_zero_nao_conta_como_erro() {
        let jsonl = r#"
{"ts":"t","run_id":"r","seq":1,"event":"tool_exec","result":"FAIL","duration_ms":10,"exit_code":1,"step":1,"msg":"git rev-parse --verify --quiet feat/x"}
{"ts":"t","run_id":"r","seq":2,"from":"pick","event":"phase_end","result":"PASS","duration_ms":20,"step":1,"msg":""}
{"ts":"t","run_id":"r","seq":3,"event":"run_end","result":"PASS","step":1,"msg":""}
"#;
        let m = derivar("r", jsonl).unwrap();
        assert_eq!(m.erros, 0, "sondagem nao e erro");
        assert_eq!(m.ferramentas_nao_zero, 1, "mas fica contada em separado");
    }

    #[test]
    fn fase_reprovada_conta_como_erro() {
        let jsonl = r#"
{"ts":"t","run_id":"r","seq":1,"from":"smoke","event":"phase_end","result":"FAIL","duration_ms":30,"step":1,"msg":""}
{"ts":"t","run_id":"r","seq":2,"from":"smoke","event":"abort","result":"HALT","step":1,"msg":"FAIL em `smoke`: engine fora"}
{"ts":"t","run_id":"r","seq":3,"event":"run_end","result":"HALT","step":1,"msg":""}
"#;
        let m = derivar("r", jsonl).unwrap();
        assert_eq!(m.erros, 1);
        assert_eq!(m.abortos, 1);
        assert_eq!(m.resultado, "HALT");
        assert!(m.motivo_da_parada.unwrap().contains("engine fora"));
    }

    /// "Onde travou" tem de sair da metrica sem abrir o trace.
    #[test]
    fn run_bloqueado_carrega_o_motivo() {
        let jsonl = r#"
{"ts":"t","run_id":"r","seq":1,"from":"implement","event":"blocked","result":"HALT","step":6,"msg":"2 lacuna(s) aguardando decisao humana"}
{"ts":"t","run_id":"r","seq":2,"event":"run_end","result":"HALT","step":6,"msg":""}
"#;
        let m = derivar("r", jsonl).unwrap();
        assert_eq!(m.bloqueios, 1);
        assert_eq!(m.passos, 6);
        assert!(m.motivo_da_parada.unwrap().contains("lacuna"));
    }

    /// Interrompido por fora nao e reprovado: sem `run_end`, o resultado e
    /// desconhecido, e chamar isso de FAIL erraria a contagem de erros.
    #[test]
    fn run_sem_run_end_e_incompleto_e_nao_falha() {
        let jsonl = r#"
{"ts":"t","run_id":"r","seq":1,"from":"start","event":"phase_end","result":"PASS","duration_ms":1,"step":1,"msg":""}
"#;
        let m = derivar("r", jsonl).unwrap();
        assert_eq!(m.resultado, "INCOMPLETO");
        assert_eq!(m.erros, 0);
    }

    /// Processo morto no meio de uma escrita deixa a ultima linha pela metade.
    /// O que ja estava gravado continua medivel.
    #[test]
    fn ultima_linha_truncada_nao_perde_o_run() {
        let jsonl = "{\"ts\":\"t\",\"run_id\":\"r\",\"seq\":1,\"from\":\"start\",\
                     \"event\":\"phase_end\",\"result\":\"PASS\",\"duration_ms\":7,\"step\":1,\
                     \"msg\":\"\"}\n{\"ts\":\"t\",\"run_id\":\"r\",\"se";
        let m = derivar("r", jsonl).unwrap();
        assert_eq!(m.fases, 1);
        assert_eq!(m.duracao_ms, 7);
        assert!(m.motivo_da_parada.unwrap().contains("truncado"));
    }

    /// Linha corrompida no meio do arquivo e outra coisa: nao da para saber o
    /// que se perdeu, e medir por cima seria inventar numero.
    #[test]
    fn linha_quebrada_no_meio_e_erro() {
        let jsonl = "nao sou json\n{\"ts\":\"t\",\"run_id\":\"r\",\"seq\":1,\"event\":\"run_end\",\
                     \"result\":\"PASS\",\"step\":1,\"msg\":\"\"}";
        assert!(derivar("r", jsonl).is_err());
    }

    #[test]
    fn trace_vazio_e_erro_e_nao_metrica_zerada() {
        assert!(derivar("r", "\n\n").is_err());
    }

    #[test]
    fn o_resumo_soma_os_runs_e_aponta_a_fase_mais_cara() {
        let a = derivar("r1", RUN_OK).unwrap();
        let mut b = derivar("r2", RUN_OK).unwrap();
        b.resultado = "HALT".to_string();
        b.fase_mais_cara = Some("verify".to_string());
        b.fase_mais_cara_ms = 4000;

        let r = resumir(&[a, b]);
        assert_eq!(r.runs, 2);
        assert_eq!(r.por_resultado.get("PASS"), Some(&1));
        assert_eq!(r.por_resultado.get("HALT"), Some(&1));
        assert_eq!(r.ferramentas, 2);
        assert_eq!(r.fase_mais_cara(), Some(("verify", 4000)));
    }
}
