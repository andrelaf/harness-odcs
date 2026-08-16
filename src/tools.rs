//! Fronteira unica com o mundo externo.
//!
//! Nenhuma chamada de processo espalhada pelo codigo: tudo passa por aqui, e
//! por isso toda invocacao externa cai no trace com comando, exit code e
//! duracao. E o mesmo ponto que atende auditoria, medicao (Semana 3) e a trave
//! de privacidade — a saida bruta vai para `evidence/`, e o trace guarda so a
//! referencia e o hash.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
    pub stdout_sha256: String,
    pub evidence_path: Option<PathBuf>,
}

impl ToolOutcome {
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }

    /// Primeira linha nao vazia do stdout — o formato de `docker --format`.
    pub fn first_line(&self) -> String {
        self.stdout
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default()
            .to_string()
    }

    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

pub fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Executa um processo, persiste a saida bruta em `evidence_dir` e devolve o
/// resultado. Nao emite trace — quem chama e responsavel por isso, para que o
/// evento carregue o contexto da fase.
pub fn run(program: &str, args: &[&str], evidence_dir: &Path, label: &str) -> Result<ToolOutcome> {
    let started = Instant::now();
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("executando `{program}` — o binario esta no PATH?"))?;
    let duration_ms = started.elapsed().as_millis();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    let evidence_path =
        persist_evidence(evidence_dir, label, program, args, &stdout, &stderr).unwrap_or(None);

    Ok(ToolOutcome {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        exit_code,
        stdout_sha256: sha256_hex(&stdout),
        stdout,
        stderr,
        duration_ms,
        evidence_path,
    })
}

/// Cria um diretorio de evidencia onde o **container** tambem consiga escrever.
///
/// `datacontract-cli` e uma imagem distroless que roda como `nonroot`, e o
/// `--output` do lint e do export sai de dentro dela. Num runner Linux o
/// diretorio nasce do usuario do CI com modo 755, o container nao consegue criar
/// o arquivo, e o CLI reprova com exit 1 **sem deixar relatorio** — sintoma tres
/// passos distante da causa, e que acusa o contrato quando o problema e de
/// permissao.
///
/// No Docker Desktop (Windows, macOS) a traducao de sistema de arquivos ignora
/// dono e modo, entao isto nunca aparece na maquina de quem desenvolve. E o tipo
/// de divergencia que so o primeiro pull request encontra.
pub fn criar_dir_de_evidencia(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 0o777 porque nao ha como saber o uid do container antes de roda-lo, e
        // o `--user` do docker resolveria isto ao preco de um uid sem entrada em
        // `/etc/passwd` dentro de uma imagem distroless. O diretorio e efemero e
        // vive sob `evidence/`, que continua 755.
        fs::set_permissions(dir, fs::Permissions::from_mode(0o777))
            .with_context(|| format!("ajustando permissoes de {}", dir.display()))?;
    }
    Ok(())
}

fn persist_evidence(
    dir: &Path,
    label: &str,
    program: &str,
    args: &[&str],
    stdout: &str,
    stderr: &str,
) -> Result<Option<PathBuf>> {
    criar_dir_de_evidencia(dir)?;
    let path = dir.join(format!("{label}.txt"));
    let body = format!(
        "# comando: {program} {}\n# --- stdout ---\n{stdout}\n# --- stderr ---\n{stderr}",
        args.join(" ")
    );
    fs::write(&path, body).with_context(|| format!("escrevendo {}", path.display()))?;
    Ok(Some(path))
}

/// Monta a invocacao do `datacontract-cli` em container, com o mount padrao.
///
/// O caminho do host precisa ir no formato nativo do sistema. Em Git Bash no
/// Windows isso importa: passar `/c/repos/...` faz o Docker montar um caminho
/// que nao existe, e o container sobe com o diretorio vazio.
pub fn datacontract_args<'a>(image: &'a str, root: &'a str, cmd_args: &[&'a str]) -> Vec<String> {
    let mut v = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-v".to_string(),
        format!("{root}:/home/datacontract"),
        image.to_string(),
    ];
    v.extend(cmd_args.iter().map(|s| s.to_string()));
    v
}
