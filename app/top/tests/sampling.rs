// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::fixtures::snapshot;

#[test]
fn first_snapshot_uses_since_boot_counters_then_interval_deltas() {
    let mut sampler = Sampler::default();
    let first = sampler.sample(snapshot(1000, 100, 0x100000001));
    assert!(first.since_boot);
    assert_eq!(first.total_ns(), 4000);
    assert_eq!(first.processes[0].runtime_ns, 100);
    let next = sampler.sample(snapshot(1100, 160, 0x100000001));
    assert!(!next.since_boot);
    assert_eq!(next.elapsed_ns, 100);
    assert_eq!(next.total_ns(), 400);
    assert_eq!(next.processes[0].runtime_ns, 60);
    assert_eq!(next.processes[0].time_ns, 160);
}

#[test]
fn recycled_threads_do_not_inherit_deltas_and_retired_time_is_retained() {
    let mut sampler = Sampler::default();
    sampler.sample(snapshot(1000, 100, 0x100000001));
    let recycled = sampler.sample(snapshot(1100, 99, 0x200000001));
    assert_eq!(recycled.processes[0].runtime_ns, 0);
    assert_eq!(recycled.processes[0].time_ns, 199);
    let mut no_threads = snapshot(1200, 0, 0);
    no_threads.threads.clear();
    let retired = sampler.sample(no_threads);
    assert_eq!(retired.processes[0].time_ns, 199);
    assert_eq!(retired.processes[0].threads, 0);
    let mut gone = snapshot(1300, 0, 0);
    gone.processes.clear();
    gone.threads.clear();
    sampler.sample(gone);
    assert!(sampler.totals.is_empty());
}

#[test]
fn new_process_generation_and_counter_regressions_cannot_borrow_runtime() {
    let mut sampler = Sampler::default();
    sampler.sample(snapshot(1000, 100, 1));
    let reset = sampler.sample(snapshot(1100, 10, 1));
    assert_eq!(reset.processes[0].runtime_ns, 0);
    let mut other = snapshot(1200, 30, 1);
    other.processes[0].koid = 2;
    if let Some(thread) = other.threads.get_mut(&1) {
        thread.process_koid = 2;
    }
    let new = sampler.sample(other);
    assert_eq!(new.processes[0].runtime_ns, 0);
    assert_eq!(new.processes[0].time_ns, 30);
}

#[test]
fn busiest_first_with_stable_ties_and_lifetime_sort() {
    let mut rows = vec![
        ProcessSample {
            koid: 3,
            name: "a".into(),
            runtime_ns: 5,
            time_ns: 100,
            ..Default::default()
        },
        ProcessSample {
            koid: 2,
            name: "z".into(),
            runtime_ns: 50,
            ..Default::default()
        },
        ProcessSample {
            koid: 1,
            name: "b".into(),
            runtime_ns: 50,
            ..Default::default()
        },
    ];
    sort(&mut rows, Sort::Cpu);
    assert_eq!(rows.iter().map(|p| p.koid).collect::<Vec<_>>(), [1, 2, 3]);
    sort(&mut rows, Sort::Name);
    assert_eq!(rows.iter().map(|p| p.koid).collect::<Vec<_>>(), [3, 1, 2]);
    sort(&mut rows, Sort::Time);
    assert_eq!(rows[0].koid, 3);
}
