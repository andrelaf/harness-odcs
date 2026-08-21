//! Implementacao das fases.
//!
//! Cada fase e uma funcao `fn(&mut Run) -> Outcome`. Nenhuma escreve estado
//! diretamente: quem persiste e o laco em `main.rs`. Isso mantem as fases
//! substituiveis e o ponto de escrita unico.

use crate::checks;
use crate::dispatch;
use crate::flow::Phase;
use crate::state::{FeatureList, FeatureStatus, Progress};
use laudo::ctx::Ctx;
use laudo::outcome::Outcome;

/// O que a maquina de estados possui, mais o contexto que ela empresta ao
/// dominio.
///
/// A divisao nao e arrumacao: `features/` nunca leu `features` nem `progress` —
/// zero ocorrencias —, e `check` carregava os dois do disco so para preencher
/// este struct, sem jamais salva-los. O tipo agora diz o que ja era verdade.
pub struct Run {
    /// Emprestado ao dominio como `&mut Ctx`. Tudo que uma fase de dominio
    /// precisa saber sobre o mundo esta aqui, e nada do que ela nao deve mexer.
    pub ctx: Ctx,
    pub features: FeatureList,
    pub progress: Progress,
    /// `fase=RESULTADO` de cada fase ja concluida neste run. Preenchido pelo
    /// laco, e nao pelas fases: quem sabe o desfecho e quem o julga.
    ///
    /// E o registro de verificacao que o handoff leva para o commit — em
    /// particular o resultado de `verify`, que e o que prova o trabalho.
    pub resultados: Vec<String>,
}

pub fn execute(phase: Phase, run: &mut Run) -> Outcome {
    run.ctx.notes.clear();
    // O passo corrente, copiado antes de cada fase. E o que permite o `tool` do
    // dominio carimbar o trace sem enxergar o `Progress` — ler o numero e uma
    // coisa, ser dono dele e outra.
    run.ctx.step = run.progress.step_count;
    match phase {
        Phase::Start => start(run),
        Phase::Plan => plan(run),
        Phase::Bearings => bearings(run),
        Phase::Smoke => smoke(run),
        Phase::Pick => pick(run),
        Phase::Implement => dominio_ou_noop(run, Phase::Implement),
        Phase::Verify => dominio_ou_noop(run, Phase::Verify),
        Phase::Handoff => handoff(run),
        Phase::Stop => stop(run),
    }
}

fn start(run: &mut Run) -> Outcome {
    run.ctx.note(format!("run_id {}", run.ctx.tracer.run_id()));
    Outcome::Pass
}

fn plan(run: &mut Run) -> Outcome {
    let id = run.ctx.feature_id.clone();
    match run.features.get(&id) {
        Some(f) => {
            run.ctx.note(format!("feature {} — {}", f.id, f.title));
            Outcome::Pass
        }
        None => Outcome::Fail(format!("feature `{id}` nao esta na lista")),
    }
}

fn bearings(run: &mut Run) -> Outcome {
    let branch = match run.ctx.tool(
        "bearings-branch",
        "git",
        &["rev-parse", "--abbrev-ref", "HEAD"],
    ) {
        Ok(o) if o.ok() => o.first_line(),
        Ok(o) => return Outcome::Fail(format!("git rev-parse saiu com {}", o.exit_code)),
        Err(e) => return Outcome::Fail(format!("{e}")),
    };
    let head = match run
        .ctx
        .tool("bearings-head", "git", &["log", "-1", "--format=%h %s"])
    {
        Ok(o) if o.ok() => o.first_line(),
        // Repositorio sem commit ainda nao e erro de fluxo.
        Ok(_) => "sem commits".to_string(),
        Err(e) => return Outcome::Fail(format!("{e}")),
    };
    run.ctx.note(format!("branch {branch} | HEAD {head}"));
    Outcome::Pass
}

fn smoke(run: &mut Run) -> Outcome {
    let cfg = run.ctx.cfg.clone();
    let mut exec = |label: &str, program: &str, args: &[&str]| run.ctx.tool(label, program, args);
    let results = checks::environment(&cfg, &mut exec);

    let failed: Vec<&checks::Check> = results.iter().filter(|c| !c.ok).collect();
    let summary: Vec<String> = results
        .iter()
        .map(|c| {
            format!(
                "{} {} — {}",
                if c.ok { "PASS" } else { "FAIL" },
                c.name,
                c.detail
            )
        })
        .collect();
    for line in summary {
        run.ctx.note(line);
    }

    if failed.is_empty() {
        Outcome::Pass
    } else {
        Outcome::Fail(
            failed
                .iter()
                .map(|c| format!("{}: {}", c.name, c.detail))
                .collect::<Vec<_>>()
                .join("; "),
        )
    }
}

fn pick(run: &mut Run) -> Outcome {
    let id = run.ctx.feature_id.clone();
    if let Err(e) = run.features.set_status(&id, FeatureStatus::InProgress) {
        return Outcome::Fail(format!("{e}"));
    }

    // O harness nao escreve na main: cada feature tem a sua branch.
    let want = format!("feat/{id}");
    let current = match run
        .ctx
        .tool("pick-branch", "git", &["rev-parse", "--abbrev-ref", "HEAD"])
    {
        Ok(o) if o.ok() => o.first_line(),
        Ok(o) => return Outcome::Fail(format!("git rev-parse saiu com {}", o.exit_code)),
        Err(e) => return Outcome::Fail(format!("{e}")),
    };

    if current != want {
        let exists = run
            .ctx
            .tool(
                "pick-branch-existe",
                "git",
                &["rev-parse", "--verify", "--quiet", &want],
            )
            .map(|o| o.ok())
            .unwrap_or(false);
        let args: Vec<&str> = if exists {
            vec!["checkout", &want]
        } else {
            vec!["checkout", "-b", &want]
        };
        match run.ctx.tool("pick-checkout", "git", &args) {
            Ok(o) if o.ok() => run.ctx.note(format!(
                "branch {want} ({})",
                if exists { "existente" } else { "criada" }
            )),
            Ok(o) => {
                return Outcome::Fail(format!(
                    "nao consegui ir para {want}: git checkout saiu com {} ({})",
                    o.exit_code,
                    o.stderr.lines().next().unwrap_or("").trim()
                ));
            }
            Err(e) => return Outcome::Fail(format!("{e}")),
        }
    } else {
        run.ctx.note(format!("branch {want} (ja ativa)"));
    }

    Outcome::Pass
}

/// As duas fases de dominio, resolvidas na ordem da spec (secao 3):
/// implementacao da feature -> implementacao generica -> no-op `Pass`.
///
/// O nivel do meio esta vazio de proposito: nao ha nada que `implement` ou
/// `verify` facam igual para todas as features. Ele fica declarado na ordem
/// para que a proxima feature que precisar dele saiba onde entra.
///
/// O no-op no fim nao e enfeite: e ele que permite F2 a F4 atravessarem o
/// fluxo antes de existirem. O que ele nao pode fazer e passar despercebido —
/// dai a nota explicita, que foi justamente o que denunciou F1 concluida sem
/// validar nada.
fn dominio_ou_noop(run: &mut Run, phase: Phase) -> Outcome {
    if let Some(outcome) = dispatch::dispatch(&mut run.ctx, phase) {
        return outcome;
    }
    run.ctx.note(format!(
        "sem implementacao de dominio para `{phase}` em `{}` — no-op",
        run.ctx.feature_id
    ));
    Outcome::Pass
}

fn handoff(run: &mut Run) -> Outcome {
    let id = run.ctx.feature_id.clone();

    // Defesa em profundidade: mesmo que `pick` tenha falhado em trocar de
    // branch, o commit nao acontece na main.
    let branch = match run.ctx.tool(
        "handoff-branch",
        "git",
        &["rev-parse", "--abbrev-ref", "HEAD"],
    ) {
        Ok(o) if o.ok() => o.first_line(),
        Ok(o) => return Outcome::Fail(format!("git rev-parse saiu com {}", o.exit_code)),
        Err(e) => return Outcome::Fail(format!("{e}")),
    };
    if branch == "main" || branch == "master" {
        return Outcome::Fail(format!(
            "recusando commit em `{branch}` — o harness nao escreve na branch principal"
        ));
    }

    // A feature vira `done` **antes** do commit, e o estado vai para o disco
    // aqui — excecao consciente a regra de que so o laco persiste, e a unica
    // no arquivo. Sem ela o commit carregaria a lista ainda em `in_progress`,
    // e o historico do Git jamais mostraria uma feature concluida: o artefato
    // que deveria ser a evidencia registraria todo run como interrompido.
    //
    // Marcar antes e seguro porque um FAIL daqui para frente e capturado pelo
    // laco, que sobrescreve o status para `failed` e persiste.
    if let Err(e) = run.features.set_status(&id, FeatureStatus::Done) {
        return Outcome::Fail(format!("{e}"));
    }
    if let Err(e) = run.features.save(&run.ctx.cfg.feature_list_path()) {
        return Outcome::Fail(format!("persistindo feature-list antes do commit: {e}"));
    }
    if let Err(e) = run.progress.save(&run.ctx.cfg.progress_path()) {
        return Outcome::Fail(format!("persistindo progress antes do commit: {e}"));
    }

    // Escopo explicito: o handoff commita os artefatos do harness, nao a
    // arvore inteira. Nada de varrer trabalho nao relacionado para dentro.
    //
    // `contracts` entra na lista porque o contrato enriquecido e o entregavel
    // do projeto, e F4 o escreve ali depois do veredito. Deixa-lo de fora faria
    // o commit registrar a decisao em `evidence/` e nao o efeito dela — e o
    // diff do contrato e justamente o que vai para revisao humana.
    //
    // O que o commit nao alcanca: a evidencia das proprias chamadas de git
    // abaixo e as ultimas linhas do trace, que so existem depois dele. Um
    // commit nao contem o registro da sua propria criacao — essas linhas
    // entram no run seguinte.
    match run.ctx.tool(
        "handoff-add",
        "git",
        &["add", "state", "trace", "evidence", "contracts"],
    ) {
        Ok(o) if o.ok() => {}
        Ok(o) => return Outcome::Fail(format!("git add saiu com {}", o.exit_code)),
        Err(e) => return Outcome::Fail(format!("{e}")),
    }

    let staged = match run.ctx.tool(
        "handoff-staged",
        "git",
        &["diff", "--cached", "--name-only"],
    ) {
        Ok(o) => o.stdout.trim().to_string(),
        Err(e) => return Outcome::Fail(format!("{e}")),
    };

    if staged.is_empty() {
        run.ctx.note("nada novo para commitar".to_string());
    } else {
        // O corpo do commit e o handoff que o brief pede: resumo, verificacao
        // e riscos. Sem eles o historico registraria que algo aconteceu, mas
        // nao se passou nem o que ficou pendente — e o commit e onde a proxima
        // pessoa comeca a ler.
        let subject = format!("harness: handoff {id}");
        let verificacao = if run.resultados.is_empty() {
            // Acontece em `./run.sh handoff` avulso, fora do laco: nao houve
            // fase julgada neste run, e dizer isso e melhor que omitir.
            "fases: nenhuma julgada neste run (handoff avulso)".to_string()
        } else {
            format!("fases: {}", run.resultados.join(" "))
        };
        let riscos = if run.ctx.riscos.is_empty() {
            "riscos: nenhum declarado pela feature".to_string()
        } else {
            format!("riscos:\n  - {}", run.ctx.riscos.join("\n  - "))
        };
        let body = format!(
            "run_id: {}\npassos: {}/{}\n{verificacao}\n{riscos}\n\
             artefatos: state/, trace/, evidence/, contracts/",
            run.ctx.tracer.run_id(),
            run.progress.step_count,
            run.progress.max_steps
        );
        match run.ctx.tool(
            "handoff-commit",
            "git",
            &["commit", "-m", &subject, "-m", &body],
        ) {
            Ok(o) if o.ok() => {}
            Ok(o) => {
                return Outcome::Fail(format!(
                    "git commit saiu com {} ({})",
                    o.exit_code,
                    o.stdout.lines().next().unwrap_or("").trim()
                ));
            }
            Err(e) => return Outcome::Fail(format!("{e}")),
        }

        match run
            .ctx
            .tool("handoff-hash", "git", &["rev-parse", "--short", "HEAD"])
        {
            Ok(o) if o.ok() => {
                let hash = o.first_line();
                run.ctx.note(format!("commit {hash} em {branch}"));
            }
            _ => run.ctx.note(format!("commit criado em {branch}")),
        }
    }

    Outcome::Pass
}

fn stop(run: &mut Run) -> Outcome {
    run.ctx
        .note(format!("feature {} concluida", run.ctx.feature_id));
    Outcome::Pass
}
