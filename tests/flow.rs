//! A tabela de transicoes, enumerada.
//!
//! Estes testes rodam sem disco, sem container e sem git — o nucleo e puro de
//! proposito. E esta a evidencia que o criterio "determinismo testavel" exige:
//! a ordem, o teto e o comportamento em falha sao verificaveis, nao afirmados.

use harness::exit::Exit;
use harness::flow::{HaltReason, Outcome, PHASES, Phase, Transition, decide};

const TETO: u32 = 12;

#[test]
fn ordem_canonica_espelha_o_brief() {
    let nomes: Vec<&str> = PHASES.iter().map(|p| p.as_str()).collect();
    assert_eq!(
        nomes,
        vec![
            "start",
            "plan",
            "bearings",
            "smoke",
            "pick",
            "implement",
            "verify",
            "handoff",
            "stop"
        ],
        "a ordem das fases e contrato com o brief; mudar aqui e mudar a spec"
    );
}

#[test]
fn pass_avanca_exatamente_uma_fase() {
    for (i, fase) in PHASES.iter().enumerate() {
        let esperado = PHASES.get(i + 1).copied();
        match decide(*fase, Outcome::Pass, 1, TETO) {
            Transition::Advance(prox) => {
                assert_eq!(Some(prox), esperado, "avanco errado a partir de {fase}")
            }
            Transition::Complete => {
                assert_eq!(esperado, None, "completou antes da ultima fase, em {fase}")
            }
            outra => panic!("PASS em {fase} deveria avancar, veio {outra:?}"),
        }
    }
}

#[test]
fn pass_na_ultima_fase_completa_o_fluxo() {
    assert_eq!(
        decide(Phase::Stop, Outcome::Pass, 9, TETO),
        Transition::Complete
    );
}

#[test]
fn fail_para_na_propria_fase_e_nunca_avanca() {
    for fase in PHASES {
        let t = decide(fase, Outcome::Fail("motivo".into()), 1, TETO);
        match t {
            Transition::Halt(HaltReason::PhaseFailed(p, _)) => {
                assert_eq!(p, fase, "parou na fase errada");
            }
            outra => panic!("FAIL em {fase} deveria parar, veio {outra:?}"),
        }
    }
}

#[test]
fn fail_devolve_exit_1() {
    let t = decide(Phase::Smoke, Outcome::Fail("engine fora".into()), 1, TETO);
    let Transition::Halt(reason) = t else {
        panic!("esperava Halt")
    };
    assert_eq!(reason.exit(), Exit::PhaseFail);
    assert_eq!(reason.exit().code(), 1);
}

#[test]
fn blocked_para_e_devolve_exit_5() {
    let t = decide(
        Phase::Implement,
        Outcome::Blocked("reclassificacao sensivel".into()),
        3,
        TETO,
    );
    let Transition::Halt(reason) = t else {
        panic!("esperava Halt")
    };
    assert!(matches!(
        reason,
        HaltReason::AwaitingHuman(Phase::Implement, _)
    ));
    assert_eq!(reason.exit().code(), 5);
}

#[test]
fn teto_tem_precedencia_sobre_qualquer_desfecho() {
    // Regra 1 da tabela: o teto e avaliado antes de Pass, Fail ou Blocked.
    for outcome in [
        Outcome::Pass,
        Outcome::Fail("x".into()),
        Outcome::Blocked("y".into()),
    ] {
        let t = decide(Phase::Implement, outcome.clone(), TETO, TETO);
        let Transition::Halt(reason) = t else {
            panic!("no teto, {outcome:?} deveria abortar")
        };
        assert!(
            matches!(reason, HaltReason::StepCeiling { .. }),
            "no teto o motivo tem de ser StepCeiling, veio {reason:?}"
        );
        assert_eq!(reason.exit().code(), 3);
    }
}

#[test]
fn abaixo_do_teto_o_fluxo_segue() {
    let t = decide(Phase::Plan, Outcome::Pass, TETO - 1, TETO);
    assert_eq!(t, Transition::Advance(Phase::Bearings));
}

#[test]
fn nao_existe_transicao_para_tras_nem_pulo() {
    for (i, fase) in PHASES.iter().enumerate() {
        match fase.next() {
            Some(prox) => assert_eq!(prox.index(), i + 1, "salto indevido depois de {fase}"),
            None => assert_eq!(i, PHASES.len() - 1, "so a ultima fase nao tem sucessora"),
        }
    }
}

#[test]
fn fluxo_inteiro_sem_falha_consome_nove_passos() {
    let mut fase = Phase::first();
    let mut passos = 0;
    loop {
        passos += 1;
        match decide(fase, Outcome::Pass, passos, TETO) {
            Transition::Advance(prox) => fase = prox,
            Transition::Complete => break,
            Transition::Halt(r) => panic!("nao deveria parar: {}", r.message()),
        }
        assert!(passos < 50, "laco nao converge");
    }
    assert_eq!(passos, 9, "o fluxo canonico tem nove fases");
}

#[test]
fn teto_menor_que_o_fluxo_aborta_antes_do_fim() {
    // Com teto 4, o fluxo nao chega ao handoff — e isso tem de ser demonstravel.
    let teto = 4;
    let mut fase = Phase::first();
    let mut passos = 0;
    let parou_em = loop {
        passos += 1;
        match decide(fase, Outcome::Pass, passos, teto) {
            Transition::Advance(prox) => fase = prox,
            Transition::Complete => panic!("completou apesar do teto"),
            Transition::Halt(r) => break r,
        }
    };
    assert!(matches!(parou_em, HaltReason::StepCeiling { max: 4, .. }));
    assert_eq!(parou_em.exit().code(), 3);
}
