//! Contrato de saida do harness.
//!
//! Os codigos sao parte da spec: e por eles que um observador externo — outra
//! IDE, um job de CI, um script — verifica o comportamento sem ler o log.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Tudo passou.
    Pass = 0,
    /// Uma fase falhou; o fluxo parou nela.
    PhaseFail = 1,
    /// Uso incorreto do ponto de entrada.
    Usage = 2,
    /// Teto de passos atingido; abortado.
    StepCeiling = 3,
    /// Estado ausente, ilegivel ou com schema incompativel.
    BadState = 4,
    /// Bloqueado aguardando decisao humana.
    Blocked = 5,
}

impl Exit {
    pub fn code(self) -> i32 {
        self as i32
    }
}
