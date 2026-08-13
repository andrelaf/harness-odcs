//! O contrato ODCS visto pelas features de dominio.
//!
//! Mora fora de qualquer feature porque F2, F3 e F4 leem o mesmo contrato. Com
//! isto dentro de `f2_mapear`, F3 teria de importar de F2 uma coisa que nao e
//! de F2 — e F4 faria o mesmo, ate ninguem mais saber de quem e o que.
//!
//! Quem interpreta ODCS e o `datacontract-cli`, via `export jsonschema`. O
//! harness nao parseia o padrao: ler `schema[].properties[].name` do YAML
//! parece trivial ate o primeiro contrato com propriedade aninhada, e ai
//! existiriam duas interpretacoes do ODCS no repositorio — a segunda sendo a
//! errada.

use crate::phases::Run;
use crate::tools;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;

/// Caminho relativo a raiz do repositorio.
///
/// Uma string, dois usos: no host resolve com `root.join(...)`; no container
/// vai como esta, porque a raiz e montada em `/home/datacontract`, que e o
/// diretorio de trabalho do CLI. Traduzir caminho em dois lugares e onde esse
/// tipo de codigo costuma quebrar entre sistemas.
pub const CAMINHO: &str = "contracts/clientes/contract.odcs.yaml";

/// Um campo do contrato, reduzido ao que as features precisam. So metadado:
/// nome e tipo. Nenhum valor de dado entra aqui — nem poderia, o contrato nao
/// os contem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Campo {
    pub nome: String,
    pub tipo: String,
}

/// Roda o motor e devolve os campos.
///
/// O destino da evidencia leva feature **e** fase: os arquivos lado a lado sao
/// a prova de que a extracao se repete. Um destino unico faria a segunda fase
/// apagar a prova da primeira.
pub fn extrair(run: &mut Run, feature: &str, fase: &str) -> Result<Vec<Campo>, String> {
    if let Err(e) = fs::create_dir_all(&run.evidence_dir) {
        return Err(format!("criando {}: {e}", run.evidence_dir.display()));
    }

    let destino = format!(
        "evidence/{}/{feature}-campos-{fase}.json",
        run.tracer.run_id()
    );
    let saida = run
        .datacontract(
            &format!("{fase}-campos"),
            &["export", "jsonschema", CAMINHO, "--output", &destino],
        )
        .map_err(|e| format!("{e}"))?;

    let bruto = fs::read_to_string(run.cfg.root.join(&destino)).map_err(|e| {
        format!(
            "`export jsonschema` saiu com {} e nao deixou {destino} ({e})",
            saida.exit_code
        )
    })?;
    if !saida.ok() {
        return Err(format!("`export jsonschema` saiu com {}", saida.exit_code));
    }

    ler_campos(&bruto).map_err(|e| format!("{e:#}"))
}

/// Identidade da entrada, pelo mesmo motivo de F1: quando dois runs
/// discordarem, foi o contrato que mudou ou a ferramenta?
pub fn sha(run: &Run) -> Result<String, String> {
    let path = run.cfg.root.join(CAMINHO);
    fs::read_to_string(&path)
        .map(|s| tools::sha256_hex(&s))
        .map_err(|e| format!("contrato `{CAMINHO}` ilegivel em {} ({e})", path.display()))
}

/// Funcao pura: o JSON Schema exportado pelo motor em lista de campos.
///
/// `BTreeMap` fixa a ordem alfabetica sem depender da feature `preserve_order`
/// do `serde_json`. Nao e a ordem do contrato; e uma ordem **estavel**, que e o
/// que os artefatos precisam para serem comparaveis entre runs.
pub fn ler_campos(bruto: &str) -> Result<Vec<Campo>> {
    let schema: JsonSchema =
        serde_json::from_str(bruto).context("export do motor nao e JSON valido")?;

    // Zero campo nao e contrato vazio, e sinal de que o formato do export
    // mudou. Passar aqui produziria cobertura de 0/0 campos — um PASS que nao
    // prova nada.
    if schema.properties.is_empty() {
        bail!("export do motor nao trouxe nenhuma propriedade — formato inesperado");
    }

    Ok(schema
        .properties
        .into_iter()
        .map(|(nome, prop)| Campo {
            tipo: tipo_legivel(prop.tipo.as_ref()),
            nome,
        })
        .collect())
}

#[derive(serde::Deserialize)]
struct JsonSchema {
    #[serde(default)]
    properties: BTreeMap<String, PropSchema>,
}

#[derive(serde::Deserialize)]
struct PropSchema {
    #[serde(default, rename = "type")]
    tipo: Option<serde_json::Value>,
}

/// `"string"` sai `string`; `["string","null"]` sai `string|null`. O tipo entra
/// nos relatorios como contexto para quem le, nunca como criterio de decisao.
fn tipo_legivel(v: Option<&serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        _ => "desconhecido".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campos_saem_do_export_do_motor() {
        let bruto = r#"{
            "type": "object",
            "properties": {
                "cpf": {"type": "string", "description": "x"},
                "cep": {"type": ["string", "null"]}
            }
        }"#;
        let campos = ler_campos(bruto).unwrap();
        // Ordem alfabetica, estavel entre runs.
        assert_eq!(campos[0].nome, "cep");
        assert_eq!(campos[0].tipo, "string|null");
        assert_eq!(campos[1].nome, "cpf");
        assert_eq!(campos[1].tipo, "string");
    }

    /// Zero propriedade daria cobertura 0/0 — um PASS que nao prova nada.
    #[test]
    fn export_sem_propriedade_e_erro_e_nao_pass_vazio() {
        assert!(ler_campos(r#"{"type": "object", "properties": {}}"#).is_err());
    }

    #[test]
    fn export_quebrado_e_erro_e_nao_pass_silencioso() {
        assert!(ler_campos("nao sou json").is_err());
    }
}
