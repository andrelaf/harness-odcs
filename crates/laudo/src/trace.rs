//! Trace append-only em JSONL.
//!
//! JSONL e nao JSON por dois motivos: append e seguro sob interrupcao, e a
//! leitura nao depende de ferramenta externa (`jq` nao existe no ambiente
//! alvo).
//!
//! O trace e o artefato que circula. Nenhum valor de campo do contrato entra
//! aqui — so nomes, decisoes e referencias para `evidence/`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

pub fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// `<UTC compacto>-<sufixo>`. O sufixo vem dos nanossegundos do relogio: basta
/// para desempatar dois runs no mesmo segundo, sem adicionar dependencia de RNG.
pub fn new_run_id() -> String {
    let fmt = format_description!("[year][month][day]T[hour][minute][second]Z");
    let stamp = OffsetDateTime::now_utc()
        .format(&fmt)
        .unwrap_or_else(|_| "00000000T000000Z".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{stamp}-{:06x}", nanos & 0x00ff_ffff)
}

/// Um evento do trace.
///
/// `Deserialize` junto de `Serialize` porque a medicao **le** este mesmo tipo:
/// `metrics` e uma derivacao do trace, e nao uma segunda instrumentacao. Um
/// tipo de leitura proprio poderia divergir do que e escrito, e a metrica
/// passaria a medir outra coisa que nao a execucao.
///
/// `#[serde(default)]` nos opcionais porque a escrita omite campo ausente:
/// sem ele, todo evento sem `result` seria erro de parse.
#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub ts: String,
    pub run_id: String,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub step: u32,
    pub msg: String,
}

/// Campos opcionais de um evento. Evita uma funcao com dez parametros
/// posicionais, em que trocar dois de lugar passa despercebido.
#[derive(Debug, Default)]
pub struct Draft {
    pub feature: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub result: Option<String>,
    pub duration_ms: Option<u128>,
    pub exit_code: Option<i32>,
    pub step: u32,
    pub msg: String,
}

pub struct Tracer {
    file: File,
    path: PathBuf,
    run_id: String,
    seq: u64,
}

impl Tracer {
    pub fn open(dir: &Path, run_id: &str) -> Result<Self> {
        fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
        let path = dir.join(format!("{run_id}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("abrindo {}", path.display()))?;
        Ok(Tracer {
            file,
            path,
            run_id: run_id.to_string(),
            seq: 0,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn emit(&mut self, event: &str, d: Draft) -> Result<()> {
        self.seq += 1;
        let ev = Event {
            ts: now_rfc3339(),
            run_id: self.run_id.clone(),
            seq: self.seq,
            feature: d.feature,
            from: d.from,
            to: d.to,
            event: event.to_string(),
            result: d.result,
            duration_ms: d.duration_ms,
            exit_code: d.exit_code,
            step: d.step,
            msg: d.msg,
        };
        let line = serde_json::to_string(&ev)?;
        writeln!(self.file, "{line}")
            .with_context(|| format!("escrevendo em {}", self.path.display()))?;
        self.file.flush()?;
        Ok(())
    }
}
