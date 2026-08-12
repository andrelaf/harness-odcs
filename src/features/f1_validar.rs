//! F1 — Validar: o contrato ODCS e valido contra o schema.
//!
//! Spec da feature: `docs/spec-f1-validar.md`.
//!
//! Divisao entre as duas fases: `implement` **produz** (resolve o contrato,
//! registra identidade, gera o relatorio legivel), `verify` **julga** (roda o
//! lint e le o veredito da saida em JSON). Manter o julgamento fora do
//! `implement` e o que faz `./run.sh verify` valer sozinho: ele reexecuta a
//! validacao inteira sem depender de nada que o run anterior tenha deixado em
//! memoria.

use crate::flow::Outcome;
use crate::phases::Run;
use crate::tools;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

/// Caminho relativo a raiz do repositorio.
///
/// Uma string, dois usos: no host resolve com `root.join(...)`; no container
/// vai como esta, porque a raiz e montada em `/home/datacontract`, que e o
/// diretorio de trabalho do CLI. Traduzir caminho em dois lugares e onde esse
/// tipo de codigo costuma quebrar entre sistemas.
const CONTRATO: &str = "contracts/clientes/contract.odcs.yaml";

/// Resolve o contrato, registra a identidade dele e gera o relatorio legivel.
pub fn implement(run: &mut Run) -> Outcome {
    let host = run.cfg.root.join(CONTRATO);
    let raw = match fs::read_to_string(&host) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Fail(format!(
                "contrato `{CONTRATO}` ilegivel em {} ({e})",
                host.display()
            ));
        }
    };

    // Identidade do que foi validado. Sem o hash, dois runs com resultados
    // diferentes nao teriam como provar se o contrato mudou entre eles.
    let sha = tools::sha256_hex(&raw);
    run.note(format!(
        "contrato {CONTRATO} — {} bytes, sha256 {}",
        raw.len(),
        &sha[..16]
    ));

    let destino = format!("evidence/{}/f1-relatorio.html", run.tracer.run_id());
    if let Err(e) = fs::create_dir_all(&run.evidence_dir) {
        return Outcome::Fail(format!("criando {}: {e}", run.evidence_dir.display()));
    }

    match run.datacontract(
        "implement-relatorio",
        &["export", "html", CONTRATO, "--output", &destino],
    ) {
        Ok(o) if o.ok() => {
            run.note(format!("relatorio {destino}"));
            Outcome::Pass
        }
        Ok(o) => Outcome::Fail(format!(
            "`export html` saiu com {} ({})",
            o.exit_code,
            primeira_linha(&o.stderr)
        )),
        Err(e) => Outcome::Fail(format!("{e}")),
    }
}

/// Roda o lint contra o schema ODCS e devolve PASS/FAIL a partir do veredito.
pub fn verify(run: &mut Run) -> Outcome {
    let destino = format!("evidence/{}/f1-lint.json", run.tracer.run_id());
    if let Err(e) = fs::create_dir_all(&run.evidence_dir) {
        return Outcome::Fail(format!("criando {}: {e}", run.evidence_dir.display()));
    }

    // O exit code do CLI ja separa valido de invalido. Ainda assim o veredito
    // e lido do JSON: e ele que carrega o motivo, e um FAIL sem motivo nao e
    // evidencia de nada.
    let saida = match run.datacontract(
        "verify-lint",
        &[
            "lint",
            CONTRATO,
            "--output-format",
            "json",
            "--output",
            &destino,
        ],
    ) {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("{e}")),
    };

    let relatorio = run.cfg.root.join(&destino);
    let bruto = match fs::read_to_string(&relatorio) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Fail(format!(
                "lint saiu com {} e nao deixou relatorio em {destino} ({e})",
                saida.exit_code
            ));
        }
    };

    let veredito = match ler_veredito(&bruto) {
        Ok(v) => v,
        Err(e) => return Outcome::Fail(format!("{e:#}")),
    };

    run.note(format!(
        "lint {} — {} check(s), relatorio {destino}",
        if veredito.passed { "PASS" } else { "FAIL" },
        veredito.checks
    ));

    if veredito.passed && saida.ok() {
        Outcome::Pass
    } else if veredito.failures.is_empty() {
        // Veredito e exit code discordando e defeito de integracao, nao
        // contrato invalido — e vale dizer isso em vez de um FAIL mudo.
        Outcome::Fail(format!(
            "lint saiu com {} sem check reprovado no relatorio",
            saida.exit_code
        ))
    } else {
        for f in &veredito.failures {
            run.note(format!("  reprovado — {f}"));
        }
        Outcome::Fail(veredito.failures.join("; "))
    }
}

/// O veredito do lint, ja reduzido ao que o fluxo precisa decidir.
#[derive(Debug, PartialEq, Eq)]
pub struct Veredito {
    pub passed: bool,
    pub checks: usize,
    pub failures: Vec<String>,
}

/// Funcao pura: JSON do `datacontract lint` em veredito. Sem disco e sem
/// container, e por isso testavel — a alternativa seria so poder verificar o
/// caminho de falha subindo um container com contrato quebrado.
pub fn ler_veredito(bruto: &str) -> Result<Veredito> {
    let rel: RelatorioLint =
        serde_json::from_str(bruto).context("relatorio do lint nao e JSON valido")?;

    let failures: Vec<String> = rel
        .checks
        .iter()
        .filter(|c| c.result != "passed")
        .map(|c| match (c.name.as_deref(), c.reason.as_deref()) {
            (Some(n), Some(r)) => format!("{n}: {r}"),
            (Some(n), None) => n.to_string(),
            (None, Some(r)) => r.to_string(),
            (None, None) => "check reprovado sem nome nem motivo".to_string(),
        })
        .collect();

    Ok(Veredito {
        // As duas condicoes: o CLI diz `passed` e nenhum check individual
        // reprovou. Confiar so no campo agregado deixaria passar um relatorio
        // internamente inconsistente.
        passed: rel.result == "passed" && failures.is_empty(),
        checks: rel.checks.len(),
        failures,
    })
}

#[derive(Deserialize)]
struct RelatorioLint {
    result: String,
    #[serde(default)]
    checks: Vec<CheckLint>,
}

#[derive(Deserialize)]
struct CheckLint {
    #[serde(default)]
    name: Option<String>,
    result: String,
    #[serde(default)]
    reason: Option<String>,
}

fn primeira_linha(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("sem detalhe")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrato_valido_vira_pass() {
        let bruto = r#"{
            "result": "passed",
            "checks": [
                {"name": "Data contract is syntactically valid", "result": "passed", "reason": null}
            ]
        }"#;
        let v = ler_veredito(bruto).unwrap();
        assert!(v.passed);
        assert_eq!(v.checks, 1);
        assert!(v.failures.is_empty());
    }

    #[test]
    fn contrato_invalido_carrega_o_motivo() {
        let bruto = r#"{
            "result": "failed",
            "checks": [
                {"name": "Check that data contract YAML is valid", "result": "failed",
                 "reason": "data must contain ['status', 'version'] properties"}
            ]
        }"#;
        let v = ler_veredito(bruto).unwrap();
        assert!(!v.passed);
        assert_eq!(v.failures.len(), 1);
        assert!(v.failures[0].contains("must contain"));
    }

    /// Agregado dizendo `passed` com check reprovado e relatorio inconsistente.
    /// Vale FAIL: a duvida nao pode virar aprovacao.
    #[test]
    fn agregado_nao_sobrepoe_check_reprovado() {
        let bruto = r#"{
            "result": "passed",
            "checks": [{"name": "x", "result": "failed", "reason": "y"}]
        }"#;
        assert!(!ler_veredito(bruto).unwrap().passed);
    }

    #[test]
    fn json_invalido_e_erro_e_nao_pass_silencioso() {
        assert!(ler_veredito("nao sou json").is_err());
    }
}
