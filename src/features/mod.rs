//! Implementacoes de dominio, uma por feature.
//!
//! O nucleo do fluxo nao conhece nenhuma delas: `phases.rs` pergunta a este
//! modulo quem atende `(feature, fase)` e cai no no-op quando ninguem atende.
//! E o que permite acrescentar F2 sem tocar na maquina de estados.
//!
//! As features moram aqui, compiladas, e nao em `features/<id>/<fase>.sh`. A
//! spec (secao 3) descreve o primeiro nivel da resolucao como
//! `features/<feature-id>/<fase>` — o slot por feature, que aqui e a funcao
//! `features::<id>::<fase>`. Um script seria o caminho por onde regra de
//! dominio vazaria para fora do binario, contra o principio "politica em Rust,
//! shell e burro": a segunda IDE passaria a depender do shell certo estar no
//! PATH para o fluxo decidir a mesma coisa.

pub mod f1_validar;

use crate::flow::{Outcome, Phase};
use crate::phases::Run;

/// Quem atende `(feature, fase)`. `None` significa "ninguem" — o chamador
/// segue para o no-op.
pub fn dispatch(run: &mut Run, phase: Phase) -> Option<Outcome> {
    match (run.feature_id.as_str(), phase) {
        ("f1-validar", Phase::Implement) => Some(f1_validar::implement(run)),
        ("f1-validar", Phase::Verify) => Some(f1_validar::verify(run)),
        _ => None,
    }
}
