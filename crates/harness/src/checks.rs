//! Checagens de ambiente.
//!
//! Um so lugar, dois consumidores: a fase `smoke` (dentro de um run, com trace)
//! e o comando `doctor` (fora de qualquer run). Duas listas de checagem
//! divergem em duas semanas — por isso o executor entra como parametro em vez
//! de o codigo ser duplicado.

use anyhow::Result;
use laudo::config::Config;
use laudo::tools::ToolOutcome;
use serde::Serialize;

/// `Serialize` porque `doctor --json` publica esta mesma estrutura. Um tipo de
/// saida proprio para a versao JSON abriria espaco para o texto e o JSON
/// discordarem sobre o resultado de uma checagem.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

impl Check {
    fn pass(name: &str, detail: impl Into<String>) -> Self {
        Check {
            name: name.to_string(),
            ok: true,
            detail: detail.into(),
        }
    }

    fn fail(name: &str, detail: impl Into<String>) -> Self {
        Check {
            name: name.to_string(),
            ok: false,
            detail: detail.into(),
        }
    }
}

/// Assinatura do executor: rotulo da evidencia, programa, argumentos.
pub type Runner<'a> = &'a mut dyn FnMut(&str, &str, &[&str]) -> Result<ToolOutcome>;

pub fn environment(cfg: &Config, run: Runner) -> Vec<Check> {
    let mut checks = Vec::new();

    // 1. O engine de container responde.
    let engine = match run(
        "smoke-engine",
        "docker",
        &["info", "--format", "{{.ServerVersion}}"],
    ) {
        Ok(o) if o.ok() => {
            checks.push(Check::pass(
                "engine de container",
                format!("Docker {}", o.first_line()),
            ));
            true
        }
        Ok(o) => {
            checks.push(Check::fail(
                "engine de container",
                format!(
                    "`docker info` saiu com {} — o daemon esta de pe? ({})",
                    o.exit_code,
                    o.stderr.lines().next().unwrap_or("sem detalhe").trim()
                ),
            ));
            false
        }
        Err(e) => {
            checks.push(Check::fail("engine de container", format!("{e}")));
            false
        }
    };

    // 2. A imagem fixada esta presente com o digest esperado.
    //    Sem isso, `latest` poderia trocar debaixo do projeto e o mesmo
    //    contrato passaria hoje e falharia amanha, sem commit nenhum.
    if engine {
        let image = cfg.dc_image.clone();
        let args = vec![
            "image",
            "inspect",
            "--format",
            "{{index .RepoDigests 0}}",
            image.as_str(),
        ];
        match run("smoke-imagem", "docker", &args) {
            Ok(o) if o.ok() => {
                let actual = o.first_line();
                if actual.contains(&cfg.dc_digest) {
                    checks.push(Check::pass(
                        "imagem fixada",
                        format!("{} @ {}", cfg.dc_image, cfg.dc_digest),
                    ));
                } else {
                    checks.push(Check::fail(
                        "imagem fixada",
                        format!(
                            "digest divergente — esperado {}, encontrado {}",
                            cfg.dc_digest, actual
                        ),
                    ));
                }
            }
            Ok(o) => checks.push(Check::fail(
                "imagem fixada",
                format!(
                    "{} ausente — rode ./scripts/bootstrap.sh (exit {})",
                    cfg.dc_image, o.exit_code
                ),
            )),
            Err(e) => checks.push(Check::fail("imagem fixada", format!("{e}"))),
        }
    } else {
        checks.push(Check::fail(
            "imagem fixada",
            "nao verificada: engine indisponivel",
        ));
    }

    // 3. Git utilizavel — o handoff depende dele.
    match run("smoke-git", "git", &["rev-parse", "--git-dir"]) {
        Ok(o) if o.ok() => checks.push(Check::pass("repositorio git", o.first_line())),
        Ok(o) => checks.push(Check::fail(
            "repositorio git",
            format!("`git rev-parse` saiu com {}", o.exit_code),
        )),
        Err(e) => checks.push(Check::fail("repositorio git", format!("{e}"))),
    }

    // 4. Diretorios de trabalho gravaveis.
    for (name, dir) in [
        ("state/", cfg.state_dir()),
        ("trace/", cfg.trace_dir()),
        ("evidence/", cfg.evidence_dir()),
    ] {
        match std::fs::create_dir_all(&dir) {
            Ok(()) => checks.push(Check::pass(name, dir.display().to_string())),
            Err(e) => checks.push(Check::fail(name, format!("{e}"))),
        }
    }

    checks
}
