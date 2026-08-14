//! Ponto de entrada do binario.
//!
//! Traduz argumentos, executa o laco e propaga exit code. Nenhuma regra de
//! fluxo mora aqui — ela esta em `flow.rs`. Nenhuma regra mora no `run.sh`,
//! que so despacha para este binario.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use harness::checks;
use harness::config::Config;
use harness::exit::Exit;
use harness::features::contrato;
use harness::flow::{self, HaltReason, Outcome, Phase, Transition};
use harness::metrics;
use harness::phases::{self, Run};
use harness::state::{
    self, Aprovacao, Aprovacoes, FeatureList, FeatureStatus, GatePendente, Progress, RunStatus,
};
use harness::tools;
use harness::trace::{self, Draft, Tracer};
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "harness-odcs",
    about = "Harness de desenvolvimento incremental para contratos ODCS",
    long_about = "Opere pelo ponto de entrada: ./run.sh <comando>"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,

    /// Avanca uma unica transicao em vez da feature inteira.
    #[arg(long, global = true)]
    step: bool,

    /// Imprime a sequencia de transicoes sem executar nada.
    #[arg(long = "dry-run", global = true)]
    dry_run: bool,

    /// Saida legivel por maquina, em stdout. Vale para `status`, `doctor` e
    /// `metrics`; nos demais e recusada.
    #[arg(long, global = true)]
    json: bool,

    /// Qual contrato operar, relativo a raiz — por exemplo
    /// `contracts/clientes/contract.odcs.yaml`.
    ///
    /// Dispensavel enquanto houver um unico contrato no repositorio. Com dois
    /// ou mais, passa a ser obrigatoria: o harness recusa e lista, em vez de
    /// escolher por voce qual contrato classificar.
    #[arg(long, global = true, value_name = "CAMINHO")]
    contrato: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Cria ou reconcilia a lista de features.
    Plan,
    /// Executa a proxima feature pelo fluxo.
    Next,
    /// Mostra onde o trabalho parou.
    Status,
    /// Re-executa apenas a fase `verify`.
    Verify,
    /// Re-executa apenas a fase `handoff`.
    Handoff,
    /// Checa o ambiente, item a item.
    Doctor,
    /// Deriva custo, duracao, erros e resultado de `trace/`.
    Metrics,
    /// Libera uma feature bloqueada para prosseguir.
    Approve { feature: String },
    /// Devolve uma feature concluida ou falhada para `pending`.
    Reset { feature: String },
}

fn main() {
    let cli = Cli::parse();

    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("erro: {e:#}");
            std::process::exit(Exit::Usage.code());
        }
    };

    // `--json` so faz sentido onde a saida e um relatorio. Nos comandos que
    // mutam estado a saida e narrativa de progresso, e serializa-la produziria
    // um JSON que ninguem consome. Recusar em voz alta e melhor que aceitar e
    // ignorar: flag silenciosamente sem efeito e a pior das tres opcoes.
    if cli.json && !matches!(cli.cmd, Cmd::Status | Cmd::Doctor | Cmd::Metrics) {
        eprintln!(
            "--json vale para `status`, `doctor` e `metrics`; os demais comandos nao o aceitam"
        );
        std::process::exit(Exit::Usage.code());
    }

    let result = match &cli.cmd {
        Cmd::Plan => cmd_plan(&cfg),
        Cmd::Next => cmd_next(&cfg, cli.step, cli.dry_run, cli.contrato.as_deref()),
        Cmd::Status => cmd_status(&cfg, cli.json),
        Cmd::Verify => cmd_single(&cfg, Phase::Verify, cli.contrato.as_deref()),
        Cmd::Handoff => cmd_single(&cfg, Phase::Handoff, cli.contrato.as_deref()),
        Cmd::Doctor => cmd_doctor(&cfg, cli.json),
        Cmd::Metrics => cmd_metrics(&cfg, cli.json),
        Cmd::Approve { feature } => cmd_approve(&cfg, feature),
        Cmd::Reset { feature } => cmd_reset(&cfg, feature),
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("erro: {e:#}");
            std::process::exit(Exit::BadState.code());
        }
    }
}

fn cmd_plan(cfg: &Config) -> Result<i32> {
    let fl_path = cfg.feature_list_path();
    let pr_path = cfg.progress_path();

    let list = FeatureList::load_or_seed(&fl_path)?;
    list.save(&fl_path)?;
    let progress = Progress::load_or_default(&pr_path)?;
    progress.save(&pr_path)?;

    println!("plano: {}", fl_path.display());
    for f in list.ordered() {
        println!(
            "  {}  {:<16} {:<12} {}",
            f.order,
            f.id,
            f.status.as_str(),
            f.title
        );
    }
    Ok(Exit::Pass.code())
}

/// O estado, em JSON.
///
/// Reaproveita `Progress` e `Feature` em vez de declarar campos proprios: os
/// dois ja sao a forma canonica do estado, e uma segunda representacao
/// divergiria da primeira na primeira mudanca de schema.
#[derive(serde::Serialize)]
struct StatusJson<'a> {
    progress: &'a Progress,
    features: Vec<&'a state::Feature>,
}

fn cmd_status(cfg: &Config, json: bool) -> Result<i32> {
    let features = FeatureList::load_or_seed(&cfg.feature_list_path())?;
    let progress = Progress::load_or_default(&cfg.progress_path())?;

    if let Err(e) = state::validate(&features, &progress) {
        eprintln!("estado invalido: {e:#}");
        return Ok(Exit::BadState.code());
    }

    if json {
        let saida = StatusJson {
            progress: &progress,
            features: features.ordered(),
        };
        println!("{}", serde_json::to_string_pretty(&saida)?);
        return Ok(Exit::Pass.code());
    }

    println!("run_status : {:?}", progress.run_status);
    println!(
        "feature    : {}",
        progress
            .current_feature
            .clone()
            .unwrap_or_else(|| "-".into())
    );
    println!(
        "fase       : {}",
        progress
            .current_phase
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".into())
    );
    println!(
        "passos     : {}/{}",
        progress.step_count, progress.max_steps
    );
    println!(
        "ultimo run : {}",
        progress.run_id.clone().unwrap_or_else(|| "-".into())
    );
    println!(
        "resultado  : {}",
        progress.last_result.clone().unwrap_or_else(|| "-".into())
    );
    println!();
    for f in features.ordered() {
        println!(
            "  {}  {:<16} {:<12} {}",
            f.order,
            f.id,
            f.status.as_str(),
            f.title
        );
    }
    Ok(Exit::Pass.code())
}

fn cmd_doctor(cfg: &Config, json: bool) -> Result<i32> {
    let evidence = cfg.evidence_dir().join("doctor");
    let mut exec =
        |label: &str, program: &str, args: &[&str]| tools::run(program, args, &evidence, label);
    let results = checks::environment(cfg, &mut exec);
    let failed = results.iter().filter(|c| !c.ok).count();

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        // O exit code continua sendo o veredito, com ou sem `--json`: quem
        // consome em CI olha o codigo, nao conta itens.
        return Ok(if failed == 0 {
            Exit::Pass.code()
        } else {
            Exit::PhaseFail.code()
        });
    }

    for c in &results {
        println!(
            "{}  {:<22} {}",
            if c.ok { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        );
    }
    if failed == 0 {
        Ok(Exit::Pass.code())
    } else {
        eprintln!("\n{failed} checagem(ns) falharam");
        Ok(Exit::PhaseFail.code())
    }
}

/// Deriva a medicao de `trace/` e a imprime.
///
/// Regenera `metrics/metrics.jsonl` inteiro a cada chamada. Nao ha acumulo
/// incremental para corromper, e a fonte continua sendo uma so — o trace.
///
/// A saida nao e um despejo de numeros: ela responde as duas perguntas que o
/// brief cobra da medicao, "onde travou" e "onde saiu caro", porque numero sem
/// leitura nao e medicao, e enfeite.
/// A medicao, em JSON: os runs e a leitura do conjunto no mesmo objeto.
///
/// O arquivo `metrics/metrics.jsonl` continua sendo uma linha por run, sem o
/// resumo — ele e derivavel, e grava-lo criaria um numero que pode envelhecer
/// em relacao as linhas ao lado.
#[derive(serde::Serialize)]
struct MetricsJson<'a> {
    runs: &'a [metrics::MetricaDeRun],
    resumo: metrics::Resumo,
}

fn cmd_metrics(cfg: &Config, json: bool) -> Result<i32> {
    let metricas = metrics::coletar(&cfg.trace_dir())?;
    if metricas.is_empty() {
        if json {
            println!("{{\"runs\":[],\"resumo\":null}}");
        } else {
            println!("nenhum run em {} — nada a medir", cfg.trace_dir().display());
        }
        return Ok(Exit::Pass.code());
    }

    let destino = cfg.metrics_path();
    metrics::gravar(&metricas, &destino)?;

    if json {
        let saida = MetricsJson {
            runs: &metricas,
            resumo: metrics::resumir(&metricas),
        };
        println!("{}", serde_json::to_string_pretty(&saida)?);
        return Ok(Exit::Pass.code());
    }

    println!(
        "{:<26} {:<16} {:<11} {:>4} {:>9} {:>9} {:>6}",
        "run_id", "feature", "resultado", "psos", "dur_ms", "ferr_ms", "erros"
    );
    for m in &metricas {
        println!(
            "{:<26} {:<16} {:<11} {:>4} {:>9} {:>9} {:>6}",
            m.run_id,
            m.feature.as_deref().unwrap_or("-"),
            m.resultado,
            m.passos,
            m.duracao_ms,
            m.duracao_ferramentas_ms,
            m.erros
        );
    }

    let r = metrics::resumir(&metricas);
    println!("\n{} run(s) — {}", r.runs, distribuicao(&r));
    println!(
        "duracao somada  : {} ms ({:.1} s)",
        r.duracao_ms,
        r.duracao_ms as f64 / 1000.0
    );
    if let Some(f) = r.fracao_em_ferramentas() {
        println!(
            // Uma casa decimal, e nao zero: 99,6% arredondado para 100% diria
            // que o harness custa nada, o que e quase verdade e nao e verdade.
            "em ferramenta   : {} ms ({:.1}% do total, {} invocacoes)",
            r.duracao_ferramentas_ms,
            f * 100.0,
            r.ferramentas
        );
    }
    if let Some((fase, ms)) = r.fase_mais_cara() {
        println!("fase mais cara  : {fase} ({ms} ms somados)");
    }
    println!(
        "erros           : {} fase(s) reprovada(s) · {} bloqueio(s) · {} aborto(s)",
        r.erros, r.bloqueios, r.abortos
    );

    // Onde travou. Sem isto a tabela diria que houve parada, mas nao por que —
    // e o motivo e a unica parte acionavel da medicao.
    let paradas: Vec<&metrics::MetricaDeRun> = metricas
        .iter()
        .filter(|m| m.motivo_da_parada.is_some())
        .collect();
    if !paradas.is_empty() {
        println!("\nonde travou:");
        for m in paradas {
            println!(
                "  {} — {}",
                m.run_id,
                m.motivo_da_parada.as_deref().unwrap_or("")
            );
        }
    }

    println!("\nmedicao em {}", destino.display());
    println!(
        "derivada de {} — nao ha contador paralelo; apagar metrics/ nao perde nada",
        cfg.trace_dir().display()
    );
    Ok(Exit::Pass.code())
}

fn distribuicao(r: &metrics::Resumo) -> String {
    r.por_resultado
        .iter()
        .map(|(k, v)| format!("{v} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Libera uma feature bloqueada e arquiva o pedido de gate que a bloqueou.
///
/// O comando continua burro: nao recomputa classificacao nenhuma, so consome o
/// pedido que a feature deixou em `state/gate-pendente.json` e o registra em
/// `state/aprovacoes.json` com data e run. Toda a politica de o que exige gate
/// vive na feature, em Rust.
///
/// A aprovacao e gravada **pelo hash do pedido**, nao pelo nome da feature: ela
/// vale para aquele conjunto de itens. Mudou o contrato, o glossario ou o
/// catalogo, o hash muda e o gate fecha de novo — sem isso, aprovar uma lacuna
/// hoje liberaria em silencio uma despromocao de campo PII amanha.
fn cmd_approve(cfg: &Config, feature_id: &str) -> Result<i32> {
    let fl_path = cfg.feature_list_path();
    let pr_path = cfg.progress_path();
    let mut features = FeatureList::load_or_seed(&fl_path)?;
    let mut progress = Progress::load_or_default(&pr_path)?;

    match features.get(feature_id) {
        None => {
            eprintln!("feature `{feature_id}` nao existe");
            return Ok(Exit::Usage.code());
        }
        Some(f) if f.status != FeatureStatus::Blocked => {
            eprintln!(
                "feature `{feature_id}` esta em `{}` — approve so se aplica a bloqueada",
                f.status.as_str()
            );
            return Ok(Exit::Usage.code());
        }
        Some(_) => {}
    }

    let gate_path = cfg.gate_pendente_path();
    let pendente = GatePendente::load_if_exists(&gate_path)?;

    // Pedido de outra feature nao e liberado por engano: quem bloqueou tem de
    // ser quem esta sendo aprovado.
    if let Some(p) = &pendente
        && p.feature != feature_id
    {
        eprintln!(
            "o pedido de gate pendente e da feature `{}`, nao de `{feature_id}`",
            p.feature
        );
        return Ok(Exit::Usage.code());
    }

    if let Some(p) = &pendente {
        println!("pedido {} — {}", &p.gate_sha256[..16], p.resumo);
        for item in &p.itens {
            println!("  {item}");
        }

        let mut livro = Aprovacoes::load_or_default(&cfg.aprovacoes_path())?;
        livro.aprovacoes.push(Aprovacao {
            feature: p.feature.clone(),
            gate_sha256: p.gate_sha256.clone(),
            aprovado_em: trace::now_rfc3339(),
            run_id: p.run_id.clone(),
            resumo: p.resumo.clone(),
        });
        livro.save(&cfg.aprovacoes_path())?;

        // O pedido some depois de arquivado: um pedido pendente que sobrevive a
        // propria aprovacao seria aprovado duas vezes na proxima vez.
        fs::remove_file(&gate_path)
            .with_context(|| format!("removendo {}", gate_path.display()))?;
        println!(
            "aprovacao registrada em {}",
            cfg.aprovacoes_path().display()
        );
    } else {
        println!("nenhum pedido de gate pendente — liberando apenas o estado da feature");
    }

    features.set_status(feature_id, FeatureStatus::Pending)?;
    progress.run_status = RunStatus::Idle;
    progress.current_phase = None;
    features.save(&fl_path)?;
    progress.save(&pr_path)?;

    println!("feature `{feature_id}` liberada — rode ./run.sh next");
    Ok(Exit::Pass.code())
}

/// Devolve uma feature `Done` ou `Failed` para `Pending`, para reexecucao.
///
/// Existe porque sem ele a unica forma de reexecutar uma feature concluida
/// seria editar `state/feature-list.json` na mao — exatamente o que o harness
/// existe para eliminar.
///
/// Nao apaga trace nem evidencia: o historico e imutavel, reexecutar produz um
/// novo `run_id`, e comparar os dois e o que se quer poder auditar. Tambem nao
/// libera feature `Blocked` — isso e atribuicao de `approve`, e confundir os
/// dois abriria caminho para contornar o gate humano sem aprovacao.
fn cmd_reset(cfg: &Config, feature_id: &str) -> Result<i32> {
    let fl_path = cfg.feature_list_path();
    let pr_path = cfg.progress_path();
    let mut features = FeatureList::load_or_seed(&fl_path)?;
    let mut progress = Progress::load_or_default(&pr_path)?;

    let previous = match features.get(feature_id) {
        None => {
            eprintln!("feature `{feature_id}` nao existe");
            return Ok(Exit::Usage.code());
        }
        Some(f) if f.status == FeatureStatus::Blocked => {
            eprintln!(
                "feature `{feature_id}` esta bloqueada aguardando decisao humana — libere com ./run.sh approve {feature_id}"
            );
            return Ok(Exit::Usage.code());
        }
        Some(f) if f.status != FeatureStatus::Done && f.status != FeatureStatus::Failed => {
            eprintln!(
                "feature `{feature_id}` esta em `{}` — reset so se aplica a `done` ou `failed`",
                f.status.as_str()
            );
            return Ok(Exit::Usage.code());
        }
        Some(f) => f.status,
    };

    features.set_status(feature_id, FeatureStatus::Pending)?;

    // O cursor so e mexido quando aponta para a feature reaberta. Zerar o
    // progresso de outra feature seria efeito colateral que ninguem pediu.
    if progress.current_feature.as_deref() == Some(feature_id) {
        progress.run_status = RunStatus::Idle;
        progress.current_phase = None;
    }

    features.save(&fl_path)?;
    progress.save(&pr_path)?;

    println!(
        "feature `{feature_id}`: {} -> pending — rode ./run.sh next",
        previous.as_str()
    );
    println!("trace e evidencia dos runs anteriores foram preservados");
    Ok(Exit::Pass.code())
}

/// Executa uma unica fase fora do laco. Serve a `verify` e `handoff`, que a
/// spec expoe como re-executaveis para producao de evidencia.
fn cmd_single(cfg: &Config, phase: Phase, escolha: Option<&str>) -> Result<i32> {
    let fl_path = cfg.feature_list_path();
    let pr_path = cfg.progress_path();
    let mut features = FeatureList::load_or_seed(&fl_path)?;
    let progress = Progress::load_or_default(&pr_path)?;

    if let Err(e) = state::validate(&features, &progress) {
        eprintln!("estado invalido: {e:#}");
        return Ok(Exit::BadState.code());
    }

    let feature = match features.next_actionable() {
        Some(f) => f.clone(),
        None => {
            println!("nada a fazer — todas as features concluidas");
            return Ok(Exit::Pass.code());
        }
    };

    // Reexecutar fora de um run em andamento abre um run novo. Reaproveitar o
    // run_id anterior sobrescreveria a evidencia daquele run e escreveria num
    // trace ja fechado: o resultado antigo sumiria em silencio, e comparar as
    // duas execucoes — a razao de o historico existir — deixaria de ser
    // possivel.
    let run_id = match (&progress.run_id, progress.run_status) {
        (Some(id), RunStatus::Running) => id.clone(),
        _ => trace::new_run_id(),
    };
    let tracer = Tracer::open(&cfg.trace_dir(), &run_id)?;
    let evidence_dir = cfg.evidence_dir().join(&run_id);

    let mut run = Run {
        cfg: cfg.clone(),
        features: features.clone(),
        progress: progress.clone(),
        tracer,
        feature_id: feature.id.clone(),
        contrato: contrato::resolver(&cfg.root, escolha)?,
        evidence_dir,
        tool_seq: 0,
        notes: Vec::new(),
        resultados: Vec::new(),
        riscos: Vec::new(),
    };
    run.progress.run_id = Some(run_id.clone());

    // Envelope run_start/run_end tambem aqui: um arquivo de trace com formato
    // proprio viraria caso especial para a derivacao de metricas da Semana 3.
    run.tracer.emit(
        "run_start",
        Draft {
            feature: Some(feature.id.clone()),
            to: Some(phase.to_string()),
            step: run.progress.step_count,
            msg: format!("reexecucao de `{phase}`"),
            ..Default::default()
        },
    )?;

    // `phase_start` também aqui: a spec (seção 6) diz "entrada em cada fase", e
    // uma reexecução que emitisse só o `phase_end` deixaria um trace com um
    // formato para o laço e outro para o comando avulso.
    run.tracer.emit(
        "phase_start",
        Draft {
            feature: Some(feature.id.clone()),
            to: Some(phase.to_string()),
            step: run.progress.step_count,
            ..Default::default()
        },
    )?;

    let started = std::time::Instant::now();
    let outcome = phases::execute(phase, &mut run);
    let duration = started.elapsed().as_millis();
    let label = outcome_label(&outcome);
    run.resultados.push(format!("{phase}={label}"));

    println!("  {:<10} {}", phase.as_str(), label);
    let notes = run.notes.join(" | ");
    for n in run.notes.clone() {
        println!("             {n}");
    }

    run.tracer.emit(
        "phase_end",
        Draft {
            feature: Some(feature.id.clone()),
            from: Some(phase.to_string()),
            result: Some(label.to_string()),
            duration_ms: Some(duration),
            step: run.progress.step_count,
            msg: notes,
            ..Default::default()
        },
    )?;
    let step = run.progress.step_count;
    emit_run_end(&mut run, &feature.id, label, step)?;

    run.progress.last_result = Some(label.to_string());
    run.progress.last_transition_at = Some(trace::now_rfc3339());

    features = run.features.clone();
    features.save(&fl_path)?;
    run.progress.save(&pr_path)?;

    println!("trace: {}", run.tracer.path().display());

    Ok(match outcome {
        Outcome::Pass => Exit::Pass.code(),
        Outcome::Fail(_) => Exit::PhaseFail.code(),
        Outcome::Blocked(_) => Exit::Blocked.code(),
    })
}

fn cmd_next(cfg: &Config, step_mode: bool, dry_run: bool, escolha: Option<&str>) -> Result<i32> {
    let fl_path = cfg.feature_list_path();
    let pr_path = cfg.progress_path();

    let features = FeatureList::load_or_seed(&fl_path)?;
    let progress = Progress::load_or_default(&pr_path)?;

    if let Err(e) = state::validate(&features, &progress) {
        eprintln!("estado invalido: {e:#}");
        return Ok(Exit::BadState.code());
    }

    let feature = match features.next_actionable() {
        Some(f) => f.clone(),
        None => {
            println!("nada pendente — todas as features estao concluidas");
            return Ok(Exit::Pass.code());
        }
    };

    // Invariante: feature bloqueada so sai desse estado por `approve`.
    if feature.status == FeatureStatus::Blocked {
        eprintln!(
            "feature `{}` esta bloqueada aguardando decisao humana — libere com ./run.sh approve {}",
            feature.id, feature.id
        );
        return Ok(Exit::Blocked.code());
    }

    let resuming = progress.run_status == RunStatus::Running
        && progress.current_feature.as_deref() == Some(feature.id.as_str())
        && progress.current_phase.is_some();

    let mut phase = if resuming {
        progress.current_phase.unwrap_or_else(Phase::first)
    } else {
        Phase::first()
    };

    if dry_run {
        println!("dry-run — feature {} ({})", feature.id, feature.title);
        let mut p = Some(phase);
        let mut n = progress.step_count;
        while let Some(cur) = p {
            n += 1;
            println!("  {n:>2}. {cur}");
            p = cur.next();
        }
        println!("teto: {} passos", progress.max_steps);
        return Ok(Exit::Pass.code());
    }

    let run_id = if resuming {
        progress.run_id.clone().unwrap_or_else(trace::new_run_id)
    } else {
        trace::new_run_id()
    };

    let tracer = Tracer::open(&cfg.trace_dir(), &run_id)?;
    let evidence_dir = cfg.evidence_dir().join(&run_id);

    let mut run = Run {
        cfg: cfg.clone(),
        features,
        progress,
        tracer,
        feature_id: feature.id.clone(),
        contrato: contrato::resolver(&cfg.root, escolha)?,
        evidence_dir,
        tool_seq: 0,
        notes: Vec::new(),
        resultados: Vec::new(),
        riscos: Vec::new(),
    };

    run.progress.run_id = Some(run_id.clone());
    run.progress.current_feature = Some(feature.id.clone());
    run.progress.run_status = RunStatus::Running;
    if !resuming {
        run.progress.step_count = 0;
        run.progress.attempts += 1;
    }

    println!("feature {} — {}", feature.id, feature.title);
    run.tracer.emit(
        "run_start",
        Draft {
            feature: Some(feature.id.clone()),
            to: Some(phase.to_string()),
            step: run.progress.step_count,
            msg: format!("teto {} passos", run.progress.max_steps),
            ..Default::default()
        },
    )?;

    loop {
        run.progress.current_phase = Some(phase);
        run.tracer.emit(
            "phase_start",
            Draft {
                feature: Some(feature.id.clone()),
                to: Some(phase.to_string()),
                step: run.progress.step_count,
                ..Default::default()
            },
        )?;

        let started = std::time::Instant::now();
        let outcome = phases::execute(phase, &mut run);
        let duration = started.elapsed().as_millis();

        let step_now = run.progress.step_count + 1;
        run.progress.step_count = step_now;

        let label = outcome_label(&outcome);
        // Antes do `handoff`, para que ele leve ao commit o resultado de todas
        // as fases que o antecederam — `verify` inclusive, que e o que prova o
        // trabalho.
        run.resultados.push(format!("{phase}={label}"));

        println!("  {:<10} {}", phase.as_str(), label);
        let notes = run.notes.clone();
        for n in &notes {
            println!("             {n}");
        }

        run.tracer.emit(
            "phase_end",
            Draft {
                feature: Some(feature.id.clone()),
                from: Some(phase.to_string()),
                result: Some(label.to_string()),
                duration_ms: Some(duration),
                step: step_now,
                msg: notes.join(" | "),
                ..Default::default()
            },
        )?;

        run.progress.last_result = Some(label.to_string());
        run.progress.last_transition_at = Some(trace::now_rfc3339());

        match flow::decide(phase, outcome, step_now, run.progress.max_steps) {
            Transition::Advance(next_phase) => {
                run.progress.current_phase = Some(next_phase);
                persist(&run, &fl_path, &pr_path)?;
                if step_mode {
                    println!("--step: proxima fase `{next_phase}`");
                    return Ok(Exit::Pass.code());
                }
                phase = next_phase;
            }

            Transition::Halt(reason) => {
                let code = reason.exit();
                let message = reason.message();
                let event = match reason {
                    HaltReason::AwaitingHuman(..) => "blocked",
                    _ => "abort",
                };

                match reason {
                    HaltReason::AwaitingHuman(..) => {
                        run.features
                            .set_status(&feature.id, FeatureStatus::Blocked)?;
                        run.progress.run_status = RunStatus::BlockedOnHuman;
                    }
                    _ => {
                        run.features
                            .set_status(&feature.id, FeatureStatus::Failed)?;
                        run.progress.run_status = RunStatus::Failed;
                    }
                }

                run.tracer.emit(
                    event,
                    Draft {
                        feature: Some(feature.id.clone()),
                        from: Some(phase.to_string()),
                        result: Some("HALT".to_string()),
                        step: step_now,
                        msg: message.clone(),
                        ..Default::default()
                    },
                )?;
                emit_run_end(&mut run, &feature.id, "HALT", step_now)?;
                persist(&run, &fl_path, &pr_path)?;

                eprintln!("\n{message}");
                eprintln!("trace: {}", run.tracer.path().display());
                return Ok(code.code());
            }

            Transition::Complete => {
                run.progress.run_status = RunStatus::Done;
                run.progress.current_phase = None;
                emit_run_end(&mut run, &feature.id, "PASS", step_now)?;
                persist(&run, &fl_path, &pr_path)?;

                println!("\nfeature {} concluida em {step_now} passos", feature.id);
                println!("trace: {}", run.tracer.path().display());
                return Ok(Exit::Pass.code());
            }
        }
    }
}

fn emit_run_end(run: &mut Run, feature_id: &str, result: &str, step: u32) -> Result<()> {
    run.tracer.emit(
        "run_end",
        Draft {
            feature: Some(feature_id.to_string()),
            result: Some(result.to_string()),
            step,
            ..Default::default()
        },
    )
}

fn persist(run: &Run, fl_path: &Path, pr_path: &Path) -> Result<()> {
    run.features.save(fl_path)?;
    run.progress.save(pr_path)?;
    Ok(())
}

fn outcome_label(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "PASS",
        Outcome::Fail(_) => "FAIL",
        Outcome::Blocked(_) => "BLOCKED",
    }
}
