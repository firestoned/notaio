//! Pure policy logic: the knob register, the built-in profiles, the lint rules, the ceilings and the
//! compiler that turns a policy plus its profile plus its grants into a bundle.
//!
//! No I/O lives here. The controller fetches objects and calls [`evaluate`]; everything that decides
//! anything is a function of its inputs, which is what makes it testable without a cluster.

pub mod builtin;
pub mod ceilings;
pub mod evaluate;
pub mod lint;
pub mod registry;

pub use evaluate::{evaluate, EvalInput, EvalOutput, GrantResult};
pub use lint::{Finding, Severity};
