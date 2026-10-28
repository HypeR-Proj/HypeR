// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Generation-qualified deltas and retained process time across thread retirement.

use crate::cli::Sort;
use hyper_os::inspect::{CpuObservation, MemoryObservation};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct ThreadSample {
    pub process_koid: u64,
    pub thread_koid: u64,
    pub runtime_ns: u64,
    pub name: String,
    pub role: &'static str,
    pub state: &'static str,
}
pub type SampleSet = BTreeMap<u64, ThreadSample>;

#[derive(Clone, Default)]
pub struct ProcessSample {
    pub koid: u64,
    pub owner: u64,
    pub name: String,
    pub owner_name: String,
    pub state: &'static str,
    pub role: &'static str,
    pub runtime_ns: u64,
    pub time_ns: u64,
    pub threads: usize,
}

pub struct Snapshot {
    pub cpu: CpuObservation,
    pub memory: MemoryObservation,
    pub processes: Vec<ProcessSample>,
    pub threads: SampleSet,
}

pub struct Frame {
    pub cpu: CpuObservation,
    pub memory: MemoryObservation,
    pub elapsed_ns: u64,
    pub since_boot: bool,
    pub processes: Vec<ProcessSample>,
    pub threads: Vec<ProcessSample>,
}
impl Frame {
    pub fn total_ns(&self) -> u64 {
        self.cpu
            .idle_ns
            .saturating_add(self.cpu.kernel_thread_ns)
            .saturating_add(self.cpu.user_thread_ns)
            .saturating_add(self.cpu.vcpu_ns)
    }
}

#[derive(Default)]
pub struct Sampler {
    previous: Option<Snapshot>,
    // Retain observed exited-thread time while its process is still visible.
    totals: BTreeMap<u64, u64>,
}
impl Sampler {
    pub fn sample(&mut self, current: Snapshot) -> Frame {
        let previous = self.previous.as_ref();
        let since_boot = previous.is_none();
        let elapsed_ns = current
            .cpu
            .captured_at_ns
            .saturating_sub(previous.map_or(0, |old| old.cpu.captured_at_ns));
        let mut cpu = current.cpu;
        if let Some(old) = previous {
            cpu.idle_ns = cpu.idle_ns.saturating_sub(old.cpu.idle_ns);
            cpu.kernel_thread_ns = cpu
                .kernel_thread_ns
                .saturating_sub(old.cpu.kernel_thread_ns);
            cpu.user_thread_ns = cpu.user_thread_ns.saturating_sub(old.cpu.user_thread_ns);
            cpu.vcpu_ns = cpu.vcpu_ns.saturating_sub(old.cpu.vcpu_ns);
        }
        let owners: BTreeMap<_, _> = current
            .processes
            .iter()
            .map(|p| (p.koid, p.name.as_str()))
            .collect();
        self.totals.retain(|koid, _| owners.contains_key(koid));
        let mut deltas: BTreeMap<u64, (u64, usize)> = BTreeMap::new();
        let mut threads = Vec::with_capacity(current.threads.len());
        for thread in current.threads.values() {
            let old = previous
                .and_then(|p| p.threads.get(&thread.thread_koid))
                .filter(|old| old.process_koid == thread.process_koid);
            // A newly observed thread has no trustworthy interval baseline. Its
            // lifetime counter contributes to TIME+, but not interval %CPU.
            let delta = if since_boot {
                thread.runtime_ns
            } else {
                old.map_or(0, |old| thread.runtime_ns.saturating_sub(old.runtime_ns))
            };
            let entry = deltas.entry(thread.process_koid).or_default();
            entry.0 = entry.0.saturating_add(delta);
            entry.1 += 1;
            if owners.contains_key(&thread.process_koid) {
                let total = self.totals.entry(thread.process_koid).or_default();
                *total = total.saturating_add(old.map_or(thread.runtime_ns, |old| {
                    thread.runtime_ns.saturating_sub(old.runtime_ns)
                }));
            }
            threads.push(ProcessSample {
                koid: thread.thread_koid,
                owner: thread.process_koid,
                owner_name: owners
                    .get(&thread.process_koid)
                    .copied()
                    .unwrap_or(if thread.process_koid == 0 {
                        "kernel"
                    } else {
                        "<unobserved>"
                    })
                    .into(),
                name: thread.name.clone(),
                state: thread.state,
                role: thread.role,
                runtime_ns: delta,
                time_ns: thread.runtime_ns,
                threads: 1,
            });
        }
        let mut processes = current.processes.clone();
        for process in &mut processes {
            let (delta, count) = deltas.get(&process.koid).copied().unwrap_or_default();
            process.runtime_ns = delta;
            process.threads = count;
            process.time_ns = self.totals.get(&process.koid).copied().unwrap_or(0);
        }
        let frame = Frame {
            cpu,
            memory: current.memory,
            elapsed_ns,
            since_boot,
            processes,
            threads,
        };
        self.previous = Some(current);
        frame
    }
}

pub fn sort(rows: &mut [ProcessSample], order: Sort) {
    rows.sort_by(|a, b| match order {
        Sort::Cpu => b.runtime_ns.cmp(&a.runtime_ns).then(a.koid.cmp(&b.koid)),
        Sort::Name => a.name.cmp(&b.name).then(a.koid.cmp(&b.koid)),
        Sort::Koid => a.koid.cmp(&b.koid),
        Sort::Time => b.time_ns.cmp(&a.time_ns).then(a.koid.cmp(&b.koid)),
        Sort::Threads => b.threads.cmp(&a.threads).then(a.koid.cmp(&b.koid)),
    });
}

#[cfg(test)]
#[path = "../tests/sampling.rs"]
mod tests;
