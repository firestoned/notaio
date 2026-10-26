//! The parts of notaio that are a library, so they can be tested: the command line and the
//! `notaio lint` engine (ADR-0006), the signer client (ADR-0004), and the health, metrics, Events
//! and fail-open guard (ADR-0010), and the generated reference pages. The controller itself lives in
//! the binary.

pub mod cli;
pub mod events;
pub mod guard;
pub mod http;
pub mod lint;
pub mod metrics;
pub mod openbao;
pub mod reference;
