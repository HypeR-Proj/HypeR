// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read, Write};

const MAX_SNAPSHOT_BYTES: u64 = 128 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Gauge,
    Counter,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    pub kind: Kind,
    pub value: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub started_at_ns: u64,
    pub finished_at_ns: u64,
    pub heap_profiling: bool,
    pub metrics: BTreeMap<String, Metric>,
}

impl Snapshot {
    pub fn new(started_at_ns: u64, finished_at_ns: u64, heap_profiling: bool) -> Self {
        Self {
            version: 1,
            started_at_ns,
            finished_at_ns,
            heap_profiling,
            metrics: BTreeMap::new(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, kind: Kind, value: u64) {
        self.metrics.insert(name.into(), Metric { kind, value });
    }

    /// Bounded input; a baseline is data, never an instruction to the kernel.
    pub fn load(reader: impl Read) -> Result<Self, Box<dyn std::error::Error>> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_SNAPSHOT_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err("snapshot exceeds 128 KiB".into());
        }
        let snapshot: Self = serde_json::from_slice(&bytes)?;
        if snapshot.version != 1
            || snapshot.started_at_ns > snapshot.finished_at_ns
            || snapshot.metrics.len() > 512
            || snapshot.metrics.keys().any(|key| key.len() > 128)
        {
            return Err("invalid memory snapshot".into());
        }
        Ok(snapshot)
    }

    /// Reject mismatched views and backwards counters instead of reporting a
    /// reboot/reset as a decrease. This cannot identify every reboot: callers
    /// must compare samples from the same boot (normally stored under /tmp).
    pub fn validate_baseline(&self, baseline: &Self) -> Result<(), &'static str> {
        if self.version != baseline.version
            || self.heap_profiling != baseline.heap_profiling
            || self.metrics.len() != baseline.metrics.len()
            || baseline.finished_at_ns > self.started_at_ns
        {
            return Err(
                "incompatible baseline; compare snapshots from the same boot and configuration",
            );
        }
        for (name, value) in &self.metrics {
            let old = baseline
                .metrics
                .get(name)
                .ok_or("baseline metric set differs")?;
            if value.kind != old.kind {
                return Err("baseline metric type differs");
            }
            if value.kind == Kind::Counter && value.value < old.value {
                return Err("counter decreased; baseline may belong to an earlier boot");
            }
        }
        Ok(())
    }

    pub fn print(
        &self,
        baseline: Option<&Self>,
        classes: bool,
        out: &mut impl Write,
    ) -> io::Result<()> {
        writeln!(
            out,
            "Memory sample: {}..{} ns; heap profiling: {}",
            self.started_at_ns,
            self.finished_at_ns,
            if self.heap_profiling { "on" } else { "off" }
        )?;
        writeln!(
            out,
            "Values are independent samples; gauges overlap. Growth alone is not proof of a leak."
        )?;
        if baseline.is_some() {
            writeln!(
                out,
                "{:44} {:>12} {:>13}  KIND",
                "METRIC", "CURRENT", "CHANGE"
            )?;
        } else {
            writeln!(out, "{:44} {:>12}  KIND", "METRIC", "VALUE")?;
        }
        for (name, metric) in &self.metrics {
            if !classes && name.starts_with("heap_class.") {
                continue;
            }
            let kind = if metric.kind == Kind::Counter {
                "counter"
            } else {
                "gauge"
            };
            if let Some(old) = baseline.and_then(|old| old.metrics.get(name)) {
                let delta = i128::from(metric.value) - i128::from(old.value);
                if delta == 0 {
                    continue;
                }
                writeln!(out, "{name:44} {:>12} {delta:>+13}  {kind}", metric.value)?;
            } else {
                writeln!(out, "{name:44} {:>12}  {kind}", metric.value)?;
            }
        }
        if !self.heap_profiling {
            writeln!(
                out,
                "Heap classes unavailable: enable CONFIG_HEAP_PROFILING for per-size traffic and occupancy."
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/snapshot.rs"]
mod tests;
