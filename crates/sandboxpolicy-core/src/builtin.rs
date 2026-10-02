//! The four built-in profiles, transcribed from the profile matrix in section 10 of the threat
//! model. Columns are Fortress, Hardened, Standard, Lab.

use std::collections::BTreeMap;

use sandboxpolicy_api::SandboxProfileSpec;
use sandboxpolicy_types::{DataScope, KnobId, Step};

pub const PROFILE_NAMES: [&str; 4] = ["fortress", "hardened", "standard", "lab"];

/// Knob id followed by the step for Fortress, Hardened, Standard and Lab.
const MATRIX: &[(&str, [u8; 4])] = &[
    ("K1.1", [0, 0, 1, 3]),
    ("K1.2", [0, 1, 1, 2]),
    ("K1.3", [0, 1, 1, 3]),
    ("K1.4", [0, 1, 2, 3]),
    ("K2.1", [0, 1, 2, 3]),
    ("K2.2", [0, 1, 1, 2]),
    ("K2.3", [0, 0, 1, 2]),
    ("K2.4", [0, 1, 1, 2]),
    ("K2.5", [0, 0, 1, 2]),
    ("K3.1", [0, 0, 1, 2]),
    ("K3.2", [0, 1, 1, 2]),
    ("K3.3", [0, 0, 1, 2]),
    ("K4.1", [0, 0, 1, 3]),
    ("K4.2", [0, 0, 0, 2]),
    ("K4.3", [0, 0, 1, 2]),
    ("K4.4", [0, 0, 0, 1]),
    ("K4.5", [0, 0, 0, 1]),
    ("K5.1", [0, 0, 1, 2]),
    ("K5.2", [0, 0, 1, 2]),
    ("K5.3", [0, 1, 1, 2]),
    ("K5.4", [0, 0, 0, 2]),
    ("K5.5", [0, 0, 0, 1]),
    ("K5.6", [0, 0, 0, 1]),
    ("K6.1", [0, 1, 1, 2]),
    ("K6.2", [0, 0, 0, 1]),
    ("K6.3", [0, 0, 0, 1]),
    ("K7.1", [0, 0, 1, 1]),
    ("K7.2", [0, 0, 1, 2]),
    ("K7.3", [0, 0, 1, 2]),
    ("K7.4", [0, 0, 0, 1]),
    ("K8.1", [0, 0, 1, 2]),
    ("K8.2", [0, 0, 0, 1]),
    ("K8.3", [0, 0, 0, 1]),
    ("K8.4", [0, 0, 0, 0]),
];

const DESCRIPTIONS: [&str; 4] = [
    "Analysis of sensitive material with no outbound path",
    "Code and document work against internal systems, read-only",
    "Everyday engineering work",
    "Experiments and tool evaluation on non-production data",
];

/// Returns the spec for a built-in profile name, or `None` for an unknown name.
pub fn profile(name: &str) -> Option<SandboxProfileSpec> {
    let col = PROFILE_NAMES.iter().position(|n| *n == name)?;
    let knobs: BTreeMap<KnobId, Step> = MATRIX
        .iter()
        .filter_map(|(id, steps)| Step::from_index(steps[col]).map(|s| (KnobId::new(id), s)))
        .collect();
    Some(SandboxProfileSpec {
        description: DESCRIPTIONS[col].to_string(),
        data_scope: if name == "lab" {
            DataScope::LabOnly
        } else {
            DataScope::Internal
        },
        knobs,
    })
}

pub fn all() -> Vec<(&'static str, SandboxProfileSpec)> {
    PROFILE_NAMES
        .iter()
        .filter_map(|n| profile(n).map(|p| (*n, p)))
        .collect()
}
