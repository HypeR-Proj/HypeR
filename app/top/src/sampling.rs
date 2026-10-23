// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Counter deltas retain complete KOIDs, so recycled slots never inherit CPU time.

use crate::cli::Sort;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default)]
pub struct ThreadSample {
    pub process_koid: u64,
    pub thread_koid: u64,
    pub runtime_ns: u64,
}
pub type SampleSet = BTreeMap<u64, ThreadSample>;

pub struct ProcessSample {
    pub koid: u64,
    pub name: String,
    pub runtime_ns: u64,
    pub threads: usize,
}

pub fn sort(rows: &mut [ProcessSample], order: Sort) {
    rows.sort_by(|a, b| match order {
        Sort::Cpu => b.runtime_ns.cmp(&a.runtime_ns).then(a.koid.cmp(&b.koid)),
        Sort::Name => a.name.cmp(&b.name).then(a.koid.cmp(&b.koid)),
        Sort::Koid => a.koid.cmp(&b.koid),
    });
}

pub fn process_delta(process: u64, previous: &SampleSet, current: &SampleSet) -> (u64, usize) {
    let mut runtime_ns = 0_u64;
    let mut threads = 0_usize;
    for sample in current.values() {
        if sample.process_koid == process {
            runtime_ns = runtime_ns.saturating_add(
                previous
                    .get(&sample.thread_koid)
                    .map_or(0, |old| sample.runtime_ns.saturating_sub(old.runtime_ns)),
            );
            threads += 1;
        }
    }
    (runtime_ns, threads)
}

#[cfg(test)]
#[path = "../tests/sampling.rs"]
mod tests;
