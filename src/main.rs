//! Ponto de entrada do binario.
//!
//! Traduz argumentos, executa o laco e propaga exit code. Nenhuma regra de
//! fluxo mora aqui — ela esta em `flow.rs`. Nenhuma regra mora no `run.sh`,
//! que so despacha para este binario.

use anyhow::Result;
use clap::{Parser, Subcommand};
use harness::checks;
use harness::config::Config;
use harness::exit::Exit;
use harness::flow::{self, HaltReason, Outcome, Phase, Transition};
use harness::phases::{self, Run};
use harness::state::{self, FeatureList, FeatureStatus, Progress, RunStatus};
use harness::tools;
use harness::trace::{self, Draft, Tracer};
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
    /// Libera uma feature bloqueada para prosseguir.
    Approve { feature: String },
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

    let result = match &cli.cmd {
        Cmd::Plan => cmd_plan(&cfg),
        Cmd::Next => cmd_next(&cfg, cli.step, cli.dry_run),
        Cmd::Status => cmd_status(&cfg),
        Cmd::Verify => cmd_single(&cfg, Phase::Verify),
        Cmd::Handoff => cmd_single(&cfg, Phase::Handoff),
        Cmd::Doctor => cmd_doctor(&cfg),
        Cmd::Approve { feature } => cmd_approve(&cfg, feature),
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

fn cmd_status(cfg: &Config) -> Result<i32> {
    let features = FeatureList::load_or_seed(&cfg.feature_list_path())?;
    let progress = Progress::load_or_default(&cfg.progress_path())?;

    if let Err(e) = state::validate(&features, &progress) {
        eprintln!("estado invalido: {e:#}");
        return Ok(Exit::BadState.code());
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

fn cmd_doctor(cfg: &Config) -> Result<i32> {
    let evidence = cfg.evidence_dir().join("doctor");
    let mut exec =
        |label: &str, program: &str, args: &[&str]| tools::run(program, args, &evidence, label);
    let results = checks::environment(cfg, &mut exec);

    let mut failed = 0;
    for c in &results {
        println!(
            "{}  {:<22} {}",
            if c.ok { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        );
        if !c.ok {
            failed += 1;
        }
    }
    if failed == 0 {
        Ok(Exit::Pass.code())
    } else {
        eprintln!("\n{failed} checagem(ns) falharam");
        Ok(Exit::PhaseFail.code())
    }
}

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

    features.set_status(feature_id, FeatureStatus::Pending)?;
    progress.run_status = RunStatus::Idle;
    progress.current_phase = None;
    features.save(&fl_path)?;
    progress.save(&pr_path)?;

    println!("feature `{feature_id}` liberada — rode ./run.sh next");
    Ok(Exit::Pass.code())
}

/// Executa uma unica fase fora do laco. Serve a `verify` e `handoff`, que a
/// spec expoe como re-executaveis para producao de evidencia.
fn cmd_single(cfg: &Config, phase: Phase) -> Result<i32> {
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

    let run_id = progress.run_id.clone().unwrap_or_else(trace::new_run_id);
    let tracer = Tracer::open(&cfg.trace_dir(), &run_id)?;
    let evidence_dir = cfg.evidence_dir().join(&run_id);

    let mut run = Run {
        cfg: cfg.clone(),
        features: features.clone(),
        progress: progress.clone(),
        tracer,
        feature_id: feature.id.clone(),
        evidence_dir,
        tool_seq: 0,
        notes: Vec::new(),
    };

    let started = std::time::Instant::now();
    let outcome = phases::execute(phase, &mut run);
    let duration = started.elapsed().as_millis();
    let label = outcome_label(&outcome);

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

    features = run.features.clone();
    features.save(&fl_path)?;
    run.progress.save(&pr_path)?;

    Ok(match outcome {
        Outcome::Pass => Exit::Pass.code(),
        Outcome::Fail(_) => Exit::PhaseFail.code(),
        Outcome::Blocked(_) => Exit::Blocked.code(),
    })
}

fn cmd_next(cfg: &Config, step_mode: bool, dry_run: bool) -> Result<i32> {
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
        evidence_dir,
        tool_seq: 0,
        notes: Vec::new(),
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
