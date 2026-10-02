//! Numeric ceilings that hang off knob steps.
//!
//! The threat model calls these "starting points to tune, not commitments", so they are data with
//! a default, not scattered constants. Only values stated in the model are filled in: K2.4 medium
//! and large sizes are not defined yet, so resource limits are not checked here.

use sandboxpolicy_types::{Budgets, Lease, Step};

#[derive(Debug, Clone)]
pub struct Ceilings {
    /// K5.1 token TTL in seconds for R0, R1, R2.
    pub token_ttl_seconds: [u32; 3],
    /// K4.3 egress bytes per lease for R0, R1, R2.
    pub egress_bytes: [u64; 3],
    /// K7.3 lease duration in seconds for R0, R1, R2.
    pub lease_seconds: [u32; 3],
    /// K7.3 concurrent sandboxes per user for R0, R1, R2.
    pub lease_concurrent: [u8; 3],
    /// Default tool call budget when a policy does not set one. Not tied to a knob.
    pub default_max_tool_calls: u32,
}

impl Default for Ceilings {
    fn default() -> Self {
        Ceilings {
            token_ttl_seconds: [5 * 60, 15 * 60, 60 * 60],
            egress_bytes: [10 << 20, 100 << 20, 1 << 30],
            lease_seconds: [4 * 3600, 8 * 3600, 24 * 3600],
            lease_concurrent: [1, 2, 5],
            default_max_tool_calls: 500,
        }
    }
}

fn idx(step: Step) -> usize {
    (step.index() as usize).min(2)
}

impl Ceilings {
    pub fn token_ttl(&self, k51: Step) -> u32 {
        self.token_ttl_seconds[idx(k51)]
    }
    pub fn egress_limit(&self, k43: Step) -> u64 {
        self.egress_bytes[idx(k43)]
    }
    pub fn lease_limit(&self, k73: Step) -> Lease {
        Lease {
            max_duration_seconds: self.lease_seconds[idx(k73)],
            max_concurrent: self.lease_concurrent[idx(k73)],
        }
    }
    pub fn default_budgets(&self, k73: Step, k43: Step) -> Budgets {
        let lease = self.lease_limit(k73);
        Budgets {
            max_tool_calls: self.default_max_tool_calls,
            max_wall_clock_seconds: lease.max_duration_seconds,
            max_egress_bytes: self.egress_limit(k43),
        }
    }
}
