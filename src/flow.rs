//! A maquina de estados. Este arquivo **e** a politica do harness.
//!
//! O nucleo (`decide`) e uma funcao pura: sem disco, sem rede, sem git. E isso
//! que permite a `tests/flow.rs` enumerar a tabela inteira de transicoes, que e
//! a prova exigida pelo criterio de determinismo.

use crate::exit::Exit;
use serde::{Deserialize, Serialize};
use std::fmt;

/// As nove fases, na ordem canonica do brief.
///
/// `Start` e `Stop` sao variantes reais e nao bordas implicitas: o enum precisa
/// poder ser lido lado a lado com o brief sem interpretacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Start,
    Plan,
    Bearings,
    Smoke,
    Pick,
    Implement,
    Verify,
    Handoff,
    Stop,
}

/// Fonte unica da ordem. Nao ha caminho condicional, pulo de fase nem retorno.
pub const PHASES: [Phase; 9] = [
    Phase::Start,
    Phase::Plan,
    Phase::Bearings,
    Phase::Smoke,
    Phase::Pick,
    Phase::Implement,
    Phase::Verify,
    Phase::Handoff,
    Phase::Stop,
];

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Start => "start",
            Phase::Plan => "plan",
            Phase::Bearings => "bearings",
            Phase::Smoke => "smoke",
            Phase::Pick => "pick",
            Phase::Implement => "implement",
            Phase::Verify => "verify",
            Phase::Handoff => "handoff",
            Phase::Stop => "stop",
        }
    }

    pub fn parse(s: &str) -> Option<Phase> {
        PHASES.iter().copied().find(|p| p.as_str() == s)
    }

    pub fn index(self) -> usize {
        PHASES
            .iter()
            .position(|p| *p == self)
            .expect("toda fase pertence a PHASES")
    }

    /// A proxima fase na ordem canonica. `None` significa fim do fluxo.
    pub fn next(self) -> Option<Phase> {
        PHASES.get(self.index() + 1).copied()
    }

    pub fn first() -> Phase {
        PHASES[0]
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// O que uma fase devolve ao fluxo.
///
/// `Blocked` existe desde o primeiro dia porque F4 exige pausa para aprovacao
/// humana em reclassificacao sensivel. Enfiar uma pausa depois num fluxo
/// binario seria refatoracao do nucleo; declarar o terceiro desfecho agora
/// custa zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String),
    Blocked(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HaltReason {
    PhaseFailed(Phase, String),
    StepCeiling { at: Phase, max: u32 },
    AwaitingHuman(Phase, String),
}

impl HaltReason {
    pub fn exit(&self) -> Exit {
        match self {
            HaltReason::PhaseFailed(..) => Exit::PhaseFail,
            HaltReason::StepCeiling { .. } => Exit::StepCeiling,
            HaltReason::AwaitingHuman(..) => Exit::Blocked,
        }
    }

    pub fn phase(&self) -> Phase {
        match self {
            HaltReason::PhaseFailed(p, _) => *p,
            HaltReason::StepCeiling { at, .. } => *at,
            HaltReason::AwaitingHuman(p, _) => *p,
        }
    }

    pub fn message(&self) -> String {
        match self {
            HaltReason::PhaseFailed(p, r) => format!("FAIL em `{p}`: {r}"),
            HaltReason::StepCeiling { at, max } => {
                format!("teto de {max} passos atingido em `{at}` — abortado")
            }
            HaltReason::AwaitingHuman(p, r) => {
                format!("bloqueado em `{p}` aguardando decisao humana: {r}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    Advance(Phase),
    Halt(HaltReason),
    Complete,
}

/// O nucleo da politica.
///
/// A ordem de avaliacao e parte do contrato — a primeira regra que casa vence:
///
/// 1. teto de passos estourado  -> `Halt(StepCeiling)`
/// 2. `Blocked`                 -> `Halt(AwaitingHuman)`
/// 3. `Fail`                    -> `Halt(PhaseFailed)`
/// 4. `Pass` na ultima fase     -> `Complete`
/// 5. `Pass`                    -> `Advance(proxima)`
///
/// O teto e verificado aqui dentro, e nao no laco de execucao. Espalhar essa
/// checagem a tornaria nao testavel — e o brief exige demonstra-la.
pub fn decide(current: Phase, outcome: Outcome, step: u32, max_steps: u32) -> Transition {
    if step >= max_steps {
        return Transition::Halt(HaltReason::StepCeiling {
            at: current,
            max: max_steps,
        });
    }

    match outcome {
        Outcome::Blocked(reason) => Transition::Halt(HaltReason::AwaitingHuman(current, reason)),
        Outcome::Fail(reason) => Transition::Halt(HaltReason::PhaseFailed(current, reason)),
        Outcome::Pass => match current.next() {
            Some(next) => Transition::Advance(next),
            None => Transition::Complete,
        },
    }
}
