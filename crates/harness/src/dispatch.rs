//! A costura entre o fluxo e o dominio: quem atende `(feature, fase)`.
//!
//! Mora no harness, e nao junto das implementacoes, porque e **roteamento** e
//! nao dominio. A pergunta que ela responde — "esta fase tem alguem que a
//! atenda?" — e de quem conduz; o que cada funcao faz e do outro crate.
//!
//! E a divisao que permite a seta existir: `Phase` e tipo do harness, e uma
//! tabela que casa `Phase` com funcao de dominio nao poderia morar em `laudo`
//! sem inverter a dependencia. Antes da separacao em crates, ela vivia em
//! `features/mod.rs` e carregava um `use crate::flow::Phase` que ninguem
//! percebia como acoplamento — porque nao havia fronteira para atravessar.
//!
//! O nucleo do fluxo continua nao conhecendo nenhuma feature: ele pergunta
//! aqui e cai no no-op quando a resposta e `None`. E o que permite acrescentar
//! F7 sem tocar na maquina de estados.

use crate::flow::Phase;
use laudo::ctx::Ctx;
use laudo::features::{
    f1_validar, f2_mapear, f3_classificar, f4_gate, f5_aninhado, f6_divergencia,
};
use laudo::outcome::Outcome;

/// Quem atende `(feature, fase)`. `None` significa "ninguem" — o chamador
/// segue para o no-op.
pub fn dispatch(ctx: &mut Ctx, phase: Phase) -> Option<Outcome> {
    match (ctx.feature_id.as_str(), phase) {
        ("f1-validar", Phase::Implement) => Some(f1_validar::implement(ctx)),
        ("f1-validar", Phase::Verify) => Some(f1_validar::verify(ctx)),
        ("f2-mapear", Phase::Implement) => Some(f2_mapear::implement(ctx)),
        ("f2-mapear", Phase::Verify) => Some(f2_mapear::verify(ctx)),
        ("f3-classificar", Phase::Implement) => Some(f3_classificar::implement(ctx)),
        ("f3-classificar", Phase::Verify) => Some(f3_classificar::verify(ctx)),
        ("f4-gate", Phase::Implement) => Some(f4_gate::implement(ctx)),
        ("f4-gate", Phase::Verify) => Some(f4_gate::verify(ctx)),
        ("f5-aninhado", Phase::Implement) => Some(f5_aninhado::implement(ctx)),
        ("f5-aninhado", Phase::Verify) => Some(f5_aninhado::verify(ctx)),
        ("f6-divergencia", Phase::Implement) => Some(f6_divergencia::implement(ctx)),
        ("f6-divergencia", Phase::Verify) => Some(f6_divergencia::verify(ctx)),
        _ => None,
    }
}
