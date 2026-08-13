//! Configuracao vinda do ambiente.
//!
//! Os valores nascem em `scripts/env.sh` — fonte unica. O binario os le de
//! variaveis de ambiente e **falha** se faltarem, em vez de carregar um default
//! embutido. Um default aqui seria uma segunda fonte de verdade, e a versao da
//! imagem passaria a divergir silenciosamente do que os scripts declaram.
//!
//! Efeito colateral desejado: rodar o binario direto, por fora do `run.sh`, nao
//! funciona. O ponto de entrada e unico por construcao, nao por convencao.

use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub dc_image: String,
    pub dc_digest: String,
}

fn var(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| {
        format!("variavel {name} nao definida — o harness se opera por `./run.sh`, que carrega scripts/env.sh")
    })
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Config {
            // Nativo, nao POSIX: este binario e um programa Windows quando roda
            // no Windows, e `/c/repos/...` nao existe para ele. Em Linux e
            // macOS as duas formas coincidem.
            root: PathBuf::from(var("HARNESS_ROOT_NATIVE")?),
            dc_image: var("DC_IMAGE")?,
            dc_digest: var("DC_DIGEST")?,
        })
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn trace_dir(&self) -> PathBuf {
        self.root.join("trace")
    }

    pub fn evidence_dir(&self) -> PathBuf {
        self.root.join("evidence")
    }

    /// A medicao derivada de `trace/`. Regenerada inteira a cada `metrics`, e
    /// por isso descartavel sem perda.
    pub fn metrics_path(&self) -> PathBuf {
        self.root.join("metrics").join("metrics.jsonl")
    }

    pub fn feature_list_path(&self) -> PathBuf {
        self.state_dir().join("feature-list.json")
    }

    pub fn progress_path(&self) -> PathBuf {
        self.state_dir().join("progress.json")
    }

    /// O pedido de gate aberto, quando ha um. E o unico arquivo de estado que
    /// nasce e morre dentro de um ciclo: a feature o cria ao bloquear,
    /// `approve` o consome.
    pub fn gate_pendente_path(&self) -> PathBuf {
        self.state_dir().join("gate-pendente.json")
    }

    pub fn aprovacoes_path(&self) -> PathBuf {
        self.state_dir().join("aprovacoes.json")
    }
}
