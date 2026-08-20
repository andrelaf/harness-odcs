//! Harness de desenvolvimento incremental.
//!
//! A politica do fluxo vive aqui, na biblioteca — nao no shell, nao em config
//! de IDE. O binario (`src/main.rs`) so traduz argumentos e propaga exit code.
//! Contrato completo em `docs/spec-harness.md`.

pub mod check;
pub mod checks;
pub mod config;
pub mod ctx;
pub mod exit;
pub mod features;
pub mod flow;
pub mod metrics;
pub mod phases;
pub mod state;
pub mod tools;
pub mod trace;
