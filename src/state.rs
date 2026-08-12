//! Estado persistido: a lista de features e o cursor do fluxo.
//!
//! Escrita atomica (tmp + rename) nos dois arquivos. Uma interrupcao no meio da
//! escrita nao pode deixar estado corrompido — e o exit 4 existe exatamente
//! para o caso em que deixou.

use crate::flow::Phase;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_MAX_STEPS: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    Pending,
    InProgress,
    Blocked,
    Done,
    Failed,
}

impl FeatureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FeatureStatus::Pending => "pending",
            FeatureStatus::InProgress => "in_progress",
            FeatureStatus::Blocked => "blocked",
            FeatureStatus::Done => "done",
            FeatureStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    /// Ordem explicita. Nunca inferida de posicao no array.
    pub order: u32,
    pub title: String,
    pub status: FeatureStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureList {
    pub schema_version: u32,
    pub features: Vec<Feature>,
}

impl FeatureList {
    /// As 4 features do dominio congelado. Escopo fechado: ideia nova vai para
    /// BACKLOG-FUTURO.md, nao para ca.
    pub fn seed() -> Self {
        let f = |id: &str, order: u32, title: &str| Feature {
            id: id.to_string(),
            order,
            title: title.to_string(),
            status: FeatureStatus::Pending,
        };
        FeatureList {
            schema_version: SCHEMA_VERSION,
            features: vec![
                f("f1-validar", 1, "Validar contrato contra o schema ODCS"),
                f("f2-mapear", 2, "Mapear campos ao glossario canonico"),
                f("f3-classificar", 3, "Classificar PII/LGPD por campo"),
                f("f4-gate", 4, "Gate humano + relatorio de lacunas"),
            ],
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?;
        let list: FeatureList =
            serde_json::from_str(&raw).with_context(|| format!("parseando {}", path.display()))?;
        Ok(list)
    }

    pub fn load_or_seed(path: &Path) -> Result<Self> {
        if path.exists() {
            FeatureList::load(path)
        } else {
            Ok(FeatureList::seed())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self)?)
    }

    /// Ordenada por `order`, nunca por posicao no vetor.
    pub fn ordered(&self) -> Vec<&Feature> {
        let mut v: Vec<&Feature> = self.features.iter().collect();
        v.sort_by_key(|f| f.order);
        v
    }

    /// A proxima feature a trabalhar: a primeira na ordem que ainda nao esta
    /// `Done`.
    ///
    /// Nao pula feature bloqueada nem falhada. Pular seria "avancar apesar do
    /// problema", o oposto do que o harness existe para garantir — quem trata
    /// cada caso e o chamador (bloqueada vira exit 5; falhada e retentada).
    pub fn next_actionable(&self) -> Option<&Feature> {
        self.ordered()
            .iter()
            .find(|f| f.status != FeatureStatus::Done)
            .copied()
    }

    pub fn get(&self, id: &str) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.id == id)
    }

    pub fn set_status(&mut self, id: &str, status: FeatureStatus) -> Result<()> {
        match self.get_mut(id) {
            Some(f) => {
                f.status = status;
                Ok(())
            }
            None => bail!("feature `{id}` nao existe na lista"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Nenhum run em andamento.
    Idle,
    Running,
    BlockedOnHuman,
    Failed,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub schema_version: u32,
    pub run_id: Option<String>,
    pub current_feature: Option<String>,
    pub current_phase: Option<Phase>,
    pub step_count: u32,
    /// Mora no arquivo, e nao em constante compilada: a demonstracao do abort
    /// por teto precisa ser inspecionavel e ajustavel sem recompilar.
    pub max_steps: u32,
    pub run_status: RunStatus,
    pub last_result: Option<String>,
    pub last_transition_at: Option<String>,
    pub attempts: u32,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            schema_version: SCHEMA_VERSION,
            run_id: None,
            current_feature: None,
            current_phase: None,
            step_count: 0,
            max_steps: DEFAULT_MAX_STEPS,
            run_status: RunStatus::Idle,
            last_result: None,
            last_transition_at: None,
            attempts: 0,
        }
    }
}

impl Progress {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?;
        let p: Progress =
            serde_json::from_str(&raw).with_context(|| format!("parseando {}", path.display()))?;
        Ok(p)
    }

    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Progress::load(path)
        } else {
            Ok(Progress::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self)?)
    }
}

/// Invariantes do estado. Violacao aqui e exit 4 — estado invalido nao vira
/// execucao.
pub fn validate(features: &FeatureList, progress: &Progress) -> Result<()> {
    if features.schema_version != SCHEMA_VERSION {
        bail!(
            "feature-list.json com schema_version {} — esperado {}",
            features.schema_version,
            SCHEMA_VERSION
        );
    }
    if progress.schema_version != SCHEMA_VERSION {
        bail!(
            "progress.json com schema_version {} — esperado {}",
            progress.schema_version,
            SCHEMA_VERSION
        );
    }

    let in_progress: Vec<&str> = features
        .features
        .iter()
        .filter(|f| f.status == FeatureStatus::InProgress)
        .map(|f| f.id.as_str())
        .collect();
    if in_progress.len() > 1 {
        bail!(
            "invariante violado: {} features em in_progress ({}) — o harness trabalha uma por vez",
            in_progress.len(),
            in_progress.join(", ")
        );
    }

    // Vale so para run em andamento. Depois que um run termina, step_count e
    // historico — baixar o teto para o proximo run nao torna o passado
    // invalido, e travar nisso impediria justamente a demonstracao do abort.
    if progress.run_status == RunStatus::Running && progress.step_count > progress.max_steps {
        bail!(
            "invariante violado: run em andamento com step_count {} maior que max_steps {}",
            progress.step_count,
            progress.max_steps
        );
    }

    if let Some(id) = &progress.current_feature
        && features.get(id).is_none()
    {
        bail!("progress.json aponta para feature `{id}`, que nao existe na lista");
    }

    Ok(())
}

/// Escrita atomica: grava num temporario e renomeia. `fs::rename` substitui o
/// destino tanto em Windows quanto em Unix.
fn write_atomic(path: &Path, data: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).with_context(|| format!("escrevendo {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("renomeando para {}", path.display()))?;
    Ok(())
}
