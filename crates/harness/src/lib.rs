//! Harness de desenvolvimento incremental — o andaime.
//!
//! A politica do fluxo vive aqui, na biblioteca — nao no shell, nao em config
//! de IDE. O binario (`src/main.rs`) so traduz argumentos e propaga exit code.
//! Contrato completo em `docs/spec-harness.md`.
//!
//! # O que este crate e, e o que ele nao e
//!
//! Ele conduz: ordem de fases explicita, teto de passos que **aborta**, gate
//! humano que para o fluxo, trace append-only e a medicao derivada dele. O que
//! ele **nao** faz e julgar contrato — isso e `laudo`, e a dependencia aponta
//! so nesta direcao.
//!
//! A divisao tornou visivel o que o `docs/curso.md` ja dizia em prosa: o
//! `check`, que e o que o CI executa em todo pull request, nao usa nada daqui.
//! O andaime fez o trabalho para o qual foi construido — conduzir quatro
//! features com rastro, verificacao e ponto de controle humano — e o que ficou
//! de pe depois foi o outro crate.
//!
//! [`dispatch`] e a costura entre os dois: dado `(feature, fase)`, qual funcao
//! de dominio atende. E o unico lugar do projeto que precisa saber as duas
//! coisas ao mesmo tempo.

pub mod checks;
pub mod dispatch;
pub mod flow;
pub mod metrics;
pub mod phases;
pub mod state;
