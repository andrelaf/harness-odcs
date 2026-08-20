//! Implementacoes de dominio, uma por feature.
//!
//! O nucleo do fluxo nao conhece nenhuma delas: ele pergunta a
//! `harness::dispatch` quem atende `(feature, fase)` e cai no no-op quando
//! ninguem atende. E o que permite acrescentar F2 sem tocar na maquina de
//! estados.
//!
//! A tabela de roteamento mora **do outro lado** da fronteira, e nao aqui: ela
//! casa `Phase` — tipo do harness — com funcao de dominio, e uma tabela dessas
//! neste crate inverteria a dependencia.
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
