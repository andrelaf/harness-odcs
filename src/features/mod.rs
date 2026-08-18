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

/// O contrato ODCS, lido pelo motor. Fora das features porque F2, F3 e F4
/// leem o mesmo arquivo.
pub mod contrato;

pub mod f1_validar;
pub mod f2_mapear;
pub mod f3_classificar;
pub mod f4_gate;
pub mod f5_aninhado;
pub mod f6_divergencia;

use crate::flow::{Outcome, Phase};
use crate::phases::Run;

/// Quem atende `(feature, fase)`. `None` significa "ninguem" — o chamador
/// segue para o no-op.
pub fn dispatch(run: &mut Run, phase: Phase) -> Option<Outcome> {
    match (run.feature_id.as_str(), phase) {
        ("f1-validar", Phase::Implement) => Some(f1_validar::implement(run)),
        ("f1-validar", Phase::Verify) => Some(f1_validar::verify(run)),
        ("f2-mapear", Phase::Implement) => Some(f2_mapear::implement(run)),
        ("f2-mapear", Phase::Verify) => Some(f2_mapear::verify(run)),
        ("f3-classificar", Phase::Implement) => Some(f3_classificar::implement(run)),
        ("f3-classificar", Phase::Verify) => Some(f3_classificar::verify(run)),
        ("f4-gate", Phase::Implement) => Some(f4_gate::implement(run)),
        ("f4-gate", Phase::Verify) => Some(f4_gate::verify(run)),
        ("f5-aninhado", Phase::Implement) => Some(f5_aninhado::implement(run)),
        ("f5-aninhado", Phase::Verify) => Some(f5_aninhado::verify(run)),
        ("f6-divergencia", Phase::Implement) => Some(f6_divergencia::implement(run)),
        ("f6-divergencia", Phase::Verify) => Some(f6_divergencia::verify(run)),
        _ => None,
    }
}
