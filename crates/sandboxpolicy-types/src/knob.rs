use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A relaxation step for a knob. `R0` is the locked default; higher steps are progressively looser.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum Step {
    R0,
    R1,
    R2,
    R3,
}

impl Step {
    pub fn index(self) -> u8 {
        match self {
            Step::R0 => 0,
            Step::R1 => 1,
            Step::R2 => 2,
            Step::R3 => 3,
        }
    }

    pub fn from_index(i: u8) -> Option<Step> {
        match i {
            0 => Some(Step::R0),
            1 => Some(Step::R1),
            2 => Some(Step::R2),
            3 => Some(Step::R3),
            _ => None,
        }
    }

    /// The next looser step, if any.
    pub fn next(self) -> Option<Step> {
        Step::from_index(self.index() + 1)
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R{}", self.index())
    }
}

/// Identifier of a knob in the register, for example `K4.1`. Validated against the registry by the
/// linter, not at deserialisation time, so an unknown id produces a finding instead of a parse error.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct KnobId(pub String);

impl KnobId {
    pub fn new(s: &str) -> Self {
        KnobId(s.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KnobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Approver roles named in the knob register.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum Role {
    Security,
    PlatformOwner,
    DataOwner,
    Compliance,
    ToolOwner,
    RiskOwner,
}
