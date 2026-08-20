//! F6 — a divergencia sobrevive ate alguem decidir.
//!
//! Spec em `docs/spec-f6-divergencia.md`. Como F5, esta feature nao acrescenta
//! etapa ao fluxo: ela corrige o que o **enriquecimento** faz quando o contrato
//! discorda do catalogo.
//!
//! Antes, o campo era sobrescrito com o que o catalogo diz. O item de gate era
//! reportado e nao impedia nada — a maquina resolvia a contradicao sozinha, a
//! favor do catalogo, e a declaracao humana sumia sem rastro. E o `decisao.md`
//! abre prometendo que nada contraditorio e persistido sem um humano dizer sim.
//!
//! `implement` delega ao F4, porque o produto continua sendo o contrato
//! enriquecido e o laudo. O que e proprio de F6 esta no `verify`.

use super::f4_gate;
use crate::ctx::Ctx;
use crate::outcome::Outcome;

pub fn implement(ctx: &mut Ctx) -> Outcome {
    f4_gate::implement(ctx)
}

/// Reprova se o enriquecimento voltar a sobrescrever quem discorda.
///
/// Compara o que a composicao **propoe** com o que o contrato **declara**, e
/// exige que todo campo em reclassificacao tenha sobrevivido intacto. Um
/// enriquecimento que "corrige" a divergencia passa no lint e produz laudo — por
/// isso a checagem precisa ser de fase, e nao de teste opcional.
pub fn verify(ctx: &mut Ctx) -> Outcome {
    let c = match f4_gate::compor(ctx, "verify") {
        Ok(c) => c,
        Err(e) => return Outcome::Fail(e),
    };

    let declarado = match f4_gate::declaracao_do_yaml(&c.yaml_enriquecido) {
        Ok(d) => d,
        Err(e) => return Outcome::Fail(format!("{e:#}")),
    };
    // O contrato como esta no disco — a declaracao que precisa sobreviver.
    let caminho = ctx.cfg.root.join(&ctx.contrato);
    let original = match std::fs::read_to_string(&caminho) {
        Ok(b) => match f4_gate::declaracao_do_yaml(&b) {
            Ok(d) => d,
            Err(e) => return Outcome::Fail(format!("{e:#}")),
        },
        Err(e) => return Outcome::Fail(format!("lendo {}: {e}", caminho.display())),
    };

    let divergentes: Vec<&f4_gate::CampoDoGate> = c
        .proposta
        .campos
        .iter()
        .filter(|x| x.mudanca == f4_gate::Mudanca::Reclassificacao)
        .collect();

    for d in &divergentes {
        let antes = original.get(&d.campo).map(|m| m.classification.clone());
        let depois = declarado.get(&d.campo).map(|m| m.classification.clone());
        if antes != depois {
            return Outcome::Fail(format!(
                "`{}` estava em reclassificacao e foi sobrescrito pelo enriquecimento — \
                 a divergencia tem de sobreviver ate alguem decidir",
                d.campo
            ));
        }
    }

    ctx.note(format!(
        "{} campo(s) em divergencia preservado(s) — o contrato mantem o que declarou",
        divergentes.len()
    ));
    Outcome::Pass
}
