//! The parts of notaio that are a library, so they can be tested: the command line and the
//! `notaio lint` engine (ADR-0006). The controller itself lives in the binary.

pub mod cli;
pub mod lint;
