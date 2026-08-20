//! O gate humano persistido: o pedido que uma feature deixa ao bloquear, e
//! o livro de decisoes que o libera.
//!
//! Mora no produto porque o gate **e** produto: quem decide que ha
//! divergencia, e quem a descreve em linguagem que um humano aprova, e a
//! composicao do laudo. O harness so pergunta se ha pedido aberto e escreve a
//! resposta — nao sabe o que um `reclassificacao` significa.
//!
//! Separado do estado de fluxo (`FeatureList`, `Progress`) pela mesma linha
//! que separa os dois crates: aqueles descrevem onde a **construcao** parou;
//! estes, o que um contrato pendura numa decisao humana.

use crate::persist::{SCHEMA_VERSION, write_atomic};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// O pedido de gate que uma feature deixou ao bloquear.
///
/// Mora aqui, e nao em `features/`, porque quem o consome e o comando
/// `approve` — e `main.rs` nao pode depender de dominio. A feature produz o
/// pedido; o comando so o arquiva.
///
/// `gate_sha256` e a identidade do que esta sendo submetido, e nao da feature:
/// a aprovacao vale para **aquele** conjunto de itens. Mudou o contrato, o
/// glossario ou o catalogo, o hash muda e o gate fecha de novo. Uma aprovacao
/// carimbada na feature seria um passe permanente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatePendente {
    pub schema_version: u32,
    pub feature: String,
    pub gate_sha256: String,
    pub run_id: String,
    pub criado_em: String,
    pub resumo: String,
    /// Linhas legiveis do que esta sendo submetido. Quem aprova le isto, nao
    /// um hash.
    pub itens: Vec<String>,
}

impl GatePendente {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parseando {}", path.display()))
    }

    pub fn load_if_exists(path: &Path) -> Result<Option<Self>> {
        if path.exists() {
            GatePendente::load(path).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self)?)
    }
}

/// Uma decisao humana, com o que ela cobre e quando foi tomada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aprovacao {
    pub feature: String,
    pub gate_sha256: String,
    pub aprovado_em: String,
    /// O run que submeteu o pedido — o caminho de volta para a evidencia.
    pub run_id: String,
    pub resumo: String,
}

/// O livro de aprovacoes. Append-only por convencao: uma decisao humana nao e
/// apagada, e o historico e o que responde "quem liberou isto, e quando?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aprovacoes {
    pub schema_version: u32,
    #[serde(default)]
    pub aprovacoes: Vec<Aprovacao>,
}

impl Default for Aprovacoes {
    fn default() -> Self {
        Aprovacoes {
            schema_version: SCHEMA_VERSION,
            aprovacoes: Vec::new(),
        }
    }
}

impl Aprovacoes {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Aprovacoes::default());
        }
        let raw = fs::read_to_string(path).with_context(|| format!("lendo {}", path.display()))?;
        serde_json::from_str(&raw).with_context(|| format!("parseando {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_string_pretty(self)?)
    }

    /// A aprovacao que cobre este pedido, se existir. Casa feature **e** hash:
    /// so o par identifica o que foi liberado.
    pub fn cobrindo(&self, feature: &str, gate_sha256: &str) -> Option<&Aprovacao> {
        self.aprovacoes
            .iter()
            .find(|a| a.feature == feature && a.gate_sha256 == gate_sha256)
    }
}
