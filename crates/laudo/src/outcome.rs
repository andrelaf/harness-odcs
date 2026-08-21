//! O desfecho de uma etapa de dominio.
//!
//! Mora aqui, e nao no harness, porque e o **tipo de retorno das funcoes de
//! dominio**: `f1_validar::implement`, `f4_gate::verify` e as outras devolvem
//! um `Outcome`, e um crate nao pode devolver um tipo que so existe em quem o
//! chama.
//!
//! O que ficou do outro lado e o que decide o que fazer com o desfecho — a
//! tabela de transicoes, o teto de passos, a fase em que se parou. Quem julga
//! diz `Pass`, `Fail` ou `Blocked`; quem conduz decide se isso avanca, para ou
//! aborta.

/// O que uma etapa devolve a quem a chamou.
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
