//! Classificacao de privacidade de contratos ODCS — o produto.
//!
//! Le o contrato, casa cada campo com o glossario canonico, classifica pelo
//! catalogo LGPD, compoe o que exige decisao humana e emite o laudo: o
//! documento que responde **por que cada campo foi classificado assim, e sob
//! qual criterio**.
//!
//! Deterministico e sem estado de fluxo. Duas execucoes sobre a mesma entrada
//! produzem o mesmo veredito, o mesmo enriquecimento e o mesmo nome de laudo —
//! e e isso que permite rodar `check` em N pull requests concorrentes sem que
//! disputem arquivo nenhum.
//!
//! # A seta
//!
//! Este crate **nao depende de `harness`**, e a ausencia e a garantia. As
//! funcoes daqui nunca precisaram da maquina de estados; enquanto os dois
//! moravam juntos, nada impedia a primeira linha que passasse a precisar. Hoje
//! um `use harness::` neste crate nao compila.
//!
//! O que o harness empresta ao dominio esta em [`ctx::Ctx`]: configuracao,
//! trace, evidencia e o contrato corrente. O que ele guarda para si — lista de
//! features, progresso, tabela de transicoes — nao chega aqui.

pub mod check;
pub mod config;
pub mod ctx;
pub mod exit;
pub mod features;
pub mod gate;
pub mod outcome;
pub mod persist;
pub mod tools;
pub mod trace;
