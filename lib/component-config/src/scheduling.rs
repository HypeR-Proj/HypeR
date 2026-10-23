// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduling deployment choices; no capability is minted by a declaration.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scheduling {
    pub policy: SchedulingPolicy,
    pub band: u8,
    pub role: SchedulingRole,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SchedulingPolicy {
    Fair {
        weight: u16,
        quantum_ns: u64,
        #[serde(default)]
        backend: FairBackend,
    },
    Budget {
        budget_ns: u64,
        period_ns: u64,
        burst_ns: u64,
    },
    Fifo {
        priority: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SchedulingRole {
    Ordinary,
    Priority,
    Protected,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FairBackend {
    Stride,
    RoundRobin,
    #[default]
    Default,
}

impl Scheduling {
    /// Kernel configuration also bounds enabled policies and the global budget
    /// period. Those live constraints are checked atomically when applied.
    pub fn validate(self) -> Result<(), &'static str> {
        let valid = match self.policy {
            SchedulingPolicy::Fair {
                weight, quantum_ns, ..
            } => (1..=1024).contains(&weight) && (1..=1_000_000_000).contains(&quantum_ns),
            SchedulingPolicy::Budget {
                budget_ns,
                period_ns,
                burst_ns,
            } => {
                (1..=1_000_000_000).contains(&period_ns)
                    && (1..=period_ns).contains(&budget_ns)
                    && (1..=period_ns).contains(&burst_ns)
            }
            SchedulingPolicy::Fifo { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err("invalid scheduling parameters")
        }
    }
}
