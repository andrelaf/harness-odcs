//! F1 — Validar: o contrato ODCS e valido contra o schema.
//!
//! Spec da feature: `docs/spec-f1-validar.md`.
//!
//! Divisao entre as duas fases: `implement` **prepara** (resolve o contrato e
//! registra a identidade dele), `verify` **julga e comprova** (roda o lint, le
//! o veredito e so entao materializa o relatorio legivel).
//!
//! O relatorio nasce em `verify`, e nao em `implement`, por um motivo achado no
//! uso: `datacontract export html` valida o contrato antes de exportar. Gerar
//! o relatorio antes do lint faria todo contrato invalido morrer em
//! `implement`, com a mensagem de um exportador em vez do motivo da
//! reprovacao — o julgamento aconteceria na fase errada, anunciado errado.
//!
//! Sobra `implement` fino, e isso e honesto: F1 e uma feature de verificacao. O
//! trabalho dela **e** o veredito.

use crate::features::contrato;
use crate::flow::Outcome;
use crate::ctx::Ctx;
use crate::tools::{self, ToolOutcome};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

/// Resolve o contrato e registra a identidade do que sera validado.
pub fn implement(ctx: &mut Ctx) -> Outcome {
    let alvo = ctx.contrato.clone();
    let host = ctx.cfg.root.join(&alvo);
    let raw = match fs::read_to_string(&host) {
        Ok(s) => s,
        Err(e) => {
            return Outcome::Fail(format!(
                "contrato `{alvo}` ilegivel em {} ({e})",
                host.display()
            ));
        }
    };

    // Identidade do que foi validado. Sem o hash, dois runs com resultados
    // diferentes nao teriam como provar se o contrato mudou entre eles.
    let sha = tools::sha256_hex(&raw);
    ctx.note(format!(
        "contrato {alvo} — {} bytes, sha256 {}",
        raw.len(),
        &sha[..16]
    ));
    Outcome::Pass
}

/// Roda o lint contra o schema ODCS, devolve PASS/FAIL a partir do veredito e,
/// quando passa, deixa o relatorio legivel ao lado.
pub fn verify(ctx: &mut Ctx) -> Outcome {
    let destino = format!("evidence/{}/f1-lint.json", ctx.tracer.run_id());
    if let Err(e) = tools::criar_dir_de_evidencia(&ctx.evidence_dir) {
        return Outcome::Fail(format!("{e:#}"));
    }

    // A convencao de nome e conferida antes do container: e barata, e um nome
    // errado nao melhora com o lint passando. Os defeitos dos dois sao juntados
    // e reportados de uma vez — quem abriu o PR precisa ver tudo agora, e nao
    // descobrir um problema novo a cada push.
    let alvo = ctx.contrato.clone();
    let mut defeitos = contrato::defeitos_do_caminho(&alvo);
    match fs::read_to_string(ctx.cfg.root.join(&alvo)) {
        Ok(bruto) => defeitos.extend(contrato::defeitos_da_identidade(&alvo, &bruto)),
        Err(e) => defeitos.push(format!("contrato `{alvo}` ilegivel ({e})")),
    }
    for aviso in contrato::avisos_do_caminho(&alvo) {
        ctx.note(format!("  aviso — {aviso}"));
    }

    // O exit code do CLI ja separa valido de invalido. Ainda assim o veredito
    // e lido do JSON: e ele que carrega o motivo, e um FAIL sem motivo nao e
    // evidencia de nada.
    let saida = match ctx.datacontract(
        "verify-lint",
        &[
            "lint",
            &alvo,
            "--output-format",
            "json",
            "--output",
            &destino,
        ],
    ) {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("{e}")),
    };

    let relatorio = ctx.cfg.root.join(&destino);
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

    ctx.note(format!(
        "lint {} — {} check(s), relatorio {destino}",
        if veredito.passed { "PASS" } else { "FAIL" },
        veredito.checks
    ));

    if veredito.passed && saida.ok() {
        // Nada a somar: o contrato e valido contra o padrao.
    } else if veredito.failures.is_empty() {
        // Veredito e exit code discordando e defeito de integracao, nao
        // contrato invalido — e vale dizer isso em vez de um FAIL mudo.
        defeitos.push(format!(
            "lint saiu com {} sem check reprovado no relatorio",
            saida.exit_code
        ));
    } else {
        defeitos.extend(veredito.failures.iter().cloned());
    }

    if !defeitos.is_empty() {
        for d in &defeitos {
            ctx.note(format!("  reprovado — {d}"));
        }
        return Outcome::Fail(defeitos.join("; "));
    }

    relatorio_legivel(ctx)
}

/// O relatorio que uma pessoa consegue abrir. Roda depois do veredito: o
/// exportador tambem valida, e um contrato reprovado nao chega ate aqui.
fn relatorio_legivel(ctx: &mut Ctx) -> Outcome {
    let destino = format!("evidence/{}/f1-relatorio.html", ctx.tracer.run_id());
    let alvo = ctx.contrato.clone();
    match ctx.datacontract(
        "verify-relatorio",
        &["export", "html", &alvo, "--output", &destino],
    ) {
        Ok(o) if o.ok() => {
            ctx.note(format!("relatorio {destino}"));
            Outcome::Pass
        }
        // Lint aprovou e o exportador recusou: os dois validam o mesmo
        // contrato, entao discordancia aqui e defeito de ferramenta, nao
        // contrato invalido. Vale FAIL — a evidencia prometida nao existe.
        Ok(o) => Outcome::Fail(format!(
            "lint aprovou mas `export html` saiu com {} ({})",
            o.exit_code,
            detalhe(&o)
        )),
        Err(e) => Outcome::Fail(format!("{e}")),
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

/// Primeira linha util da saida. Procura em `stderr` e depois em `stdout`
/// porque o CLI imprime o erro na tabela de stdout — so olhar stderr devolveria
/// "sem detalhe" justamente quando o detalhe importa.
fn detalhe(o: &ToolOutcome) -> String {
    [o.stderr.as_str(), o.stdout.as_str()]
        .iter()
        .flat_map(|s| s.lines())
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
