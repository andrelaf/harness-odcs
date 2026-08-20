//! O que os dois lados precisam para escrever um JSON de estado sem corromper.
//!
//! Mora no produto, e nao no harness, por uma razao de dependencia e nao de
//! afinidade: o gate — que e do produto — persiste pedido e aprovacao, e a seta
//! `harness -> laudo` proibe que ele importe do outro lado. Duplicar quinze
//! linhas seria mais barato ate o dia em que uma das copias ganhasse uma
//! verificacao a mais.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// A versao do formato dos arquivos JSON que este projeto escreve — estado do
/// fluxo, pedido de gate e livro de aprovacoes.
///
/// Uma so para os tres de proposito: eles sao lidos pelo mesmo binario, na
/// mesma versao, e tres numeros independentes seriam tres coisas para manter em
/// troca de flexibilidade que ninguem pediu.
pub const SCHEMA_VERSION: u32 = 1;

/// Escrita atomica: grava num temporario e renomeia. `fs::rename` substitui o
/// destino tanto em Windows quanto em Unix.
pub fn write_atomic(path: &Path, data: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).with_context(|| format!("escrevendo {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("renomeando para {}", path.display()))?;
    Ok(())
}
