//! O contexto de execucao que o **dominio** recebe.
//!
//! Separado do `Run` do harness porque as funcoes de dominio — validar,
//! mapear, classificar, compor o laudo — nunca precisaram da maquina de
//! estados. Elas leem configuracao, escrevem trace e evidencia, e devolvem um
//! desfecho; qual e a feature corrente, quantos passos ja se deram e o que vem
//! depois sao perguntas de quem **conduz**, nao de quem julga.
//!
//! A separacao ja existia no comportamento: `check` construia um `Run`
//! carregando `FeatureList` e `Progress` do disco so para preencher o struct, e
//! jamais os salvava. O que este modulo faz e dizer isso no tipo, para que
//! deixe de depender de ninguem esbarrar nela.
//!
//! O que fica de fora daqui e o que a maquina de estados possui: a lista de
//! features, o progresso, e os resultados por fase que o laco preenche.

use crate::config::Config;
use crate::tools::{self, ToolOutcome};
use crate::trace::{Draft, Tracer};
use anyhow::Result;
use std::path::PathBuf;

pub struct Ctx {
    pub cfg: Config,
    pub tracer: Tracer,
    pub feature_id: String,
    /// O contrato que este run opera, relativo a raiz e sempre com `/`.
    ///
    /// Resolvido uma vez, na abertura do run, e nao reconsultado depois: as
    /// fases de dominio precisam concordar sobre qual arquivo estao lendo,
    /// classificando e escrevendo. Um `descobrir` por fase deixaria `implement`
    /// e `verify` trabalhando em contratos diferentes se um arquivo aparecesse
    /// no meio do run.
    pub contrato: String,
    pub evidence_dir: PathBuf,
    pub tool_seq: u32,
    /// O passo corrente, **copiado** do progresso antes de cada fase.
    ///
    /// O trace carimba o passo em que cada ferramenta rodou, e isso e medicao
    /// do curso — sem ele, correlacionar duas invocacoes de container exige ler
    /// o arquivo inteiro. Mas carimbar o numero nao e o mesmo que ser dono
    /// dele: o dominio precisa **saber** o passo, e nao avanca-lo.
    ///
    /// Por isso um `u32` copiado em vez de uma referencia ao `Progress`. Fosse
    /// o struct inteiro, a fronteira que este modulo existe para desenhar
    /// duraria ate a primeira linha que escrevesse nele.
    pub step: u32,
    /// Linhas para o operador, impressas pela fase corrente.
    pub notes: Vec<String>,
    /// O que a feature quer que fique registrado como risco remanescente.
    ///
    /// Diferente de `notes`: notas sao do momento e somem no fim da fase;
    /// risco atravessa o run e entra no commit. Um campo classificado como
    /// lacuna nao reprova o fluxo, mas quem ler o commit daqui a seis meses
    /// precisa saber que ele passou sem classificacao.
    pub riscos: Vec<String>,
}

impl Ctx {
    /// Unico caminho para executar processo externo dentro de um run.
    pub fn tool(&mut self, label: &str, program: &str, args: &[&str]) -> Result<ToolOutcome> {
        self.tool_seq += 1;
        let labeled = format!("{:02}-{label}", self.tool_seq);
        let out = tools::run(program, args, &self.evidence_dir, &labeled)?;

        let evidence = out
            .evidence_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "-".to_string());

        self.tracer.emit(
            "tool_exec",
            Draft {
                feature: Some(self.feature_id.clone()),
                result: Some(if out.ok() { "PASS" } else { "FAIL" }.to_string()),
                duration_ms: Some(out.duration_ms),
                exit_code: Some(out.exit_code),
                step: self.step,
                // Referencia + hash. A saida bruta fica em evidence/, nunca
                // no trace: e o trace que circula.
                msg: format!(
                    "{} | evidence={} sha256={}",
                    out.command_line(),
                    evidence,
                    &out.stdout_sha256[..16]
                ),
                ..Default::default()
            },
        )?;

        Ok(out)
    }

    /// Invocacao do `datacontract-cli` no container fixado.
    ///
    /// Passa pelo mesmo `tool`, entao cai no trace com comando, exit code e
    /// duracao como qualquer outro processo — inclusive a tag da imagem, que e
    /// o que torna o run reproduzivel meses depois.
    pub fn datacontract(&mut self, label: &str, args: &[&str]) -> Result<ToolOutcome> {
        let image = self.cfg.dc_image.clone();
        let root = self.cfg.root.display().to_string();
        let montado = tools::datacontract_args(&image, &root, args);
        let refs: Vec<&str> = montado.iter().map(String::as_str).collect();
        self.tool(label, "docker", &refs)
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.notes.push(s.into());
    }

    /// Declara um risco remanescente. Sobrevive ao fim da fase e vai para o
    /// corpo do commit de `handoff`.
    pub fn risco(&mut self, s: impl Into<String>) {
        self.riscos.push(s.into());
    }
}
