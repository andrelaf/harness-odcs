//! F5 — cobertura de estrutura aninhada.
//!
//! Spec em `docs/spec-f5-aninhado.md`. Ao contrario de F1 a F4, esta feature nao
//! acrescenta uma etapa ao fluxo: ela corrige **como o contrato e lido**, e a
//! leitura e compartilhada pelas quatro anteriores.
//!
//! Por isso `implement` delega ao F4 — o produto continua sendo o contrato
//! enriquecido e o laudo, agora cobrindo a arvore inteira. O que e proprio de F5
//! esta no `verify`: as asseveracoes que reprovariam se a descida regredisse.
//!
//! O limite que ela fecha esta medido em `docs/cobertura.md`: antes de F5,
//! `contracts/pedidos/` mostrava cinco nos e nenhum era dado pessoal — CPF, nome
//! e e-mail viviam dentro de um objeto que o harness tratava como campo unico.

use super::{contrato, f4_gate};
use crate::flow::Outcome;
use crate::phases::Run;

pub fn implement(run: &mut Run) -> Outcome {
    f4_gate::implement(run)
}

/// Reprova se a arvore deixar de ser percorrida.
///
/// Nao basta o contrato passar: um contrato com objeto aninhado precisa produzir
/// **caminho** entre os campos. Sem esta checagem, uma regressao na extracao
/// voltaria a esconder dado pessoal e ainda assim emitiria laudo — que e o
/// cenario que esta feature existe para impedir.
pub fn verify(run: &mut Run) -> Outcome {
    let campos = match contrato::extrair(run, "f5-aninhado", "verify") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };

    if campos.is_empty() {
        return Outcome::Fail("nenhum campo extraido do contrato".to_string());
    }

    // Um campo cujo nome e prefixo de outro seria container reportado como
    // folha — exatamente o que `docs/cobertura.md` descreve como o pior caminho,
    // porque cadastrar esse nome no glossario cobre a subarvore inteira com uma
    // classificacao so.
    let nomes: Vec<&str> = campos.iter().map(|c| c.nome.as_str()).collect();
    for n in &nomes {
        if let Some(outro) = nomes.iter().find(|o| {
            **o != *n && (o.starts_with(&format!("{n}.")) || o.starts_with(&format!("{n}[].")))
        }) {
            return Outcome::Fail(format!(
                "`{n}` e container e foi reportado como campo — `{outro}` esta dentro dele"
            ));
        }
    }

    let aninhados = nomes.iter().filter(|n| n.contains('.')).count();
    run.note(format!(
        "{} campo(s), {} em profundidade — nenhum container reportado como folha",
        campos.len(),
        aninhados
    ));
    Outcome::Pass
}
