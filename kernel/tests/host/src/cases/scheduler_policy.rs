// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host checks for static scheduler classes and placement invariants.

#[path = "../../../../src/kernel/task/policy.rs"]
mod policy;

use hyper::cpu::CpuIndex;
use policy::{
    CpuMask, PlacementPolicy, RoundRobinPlacement, SchedulingClass, SchedulingPolicy,
    ThreadPlacement, ThreadPriority,
};

#[test]
fn initial_placement_rotates_only_through_admitted_cpus() {
    let mut indices = vec![0, 2, 5, hyper::cpu::MAX_CPUS - 1];
    indices.retain(|index| *index < hyper::cpu::MAX_CPUS);
    indices.sort_unstable();
    indices.dedup();
    let cpus: Vec<_> = indices
        .into_iter()
        .map(|index| crate::require_some(CpuIndex::new(index)))
        .collect();
    let online = cpus
        .iter()
        .fold(CpuMask::EMPTY, |mask, cpu| mask.with_cpu(*cpu));
    let mut placement = RoundRobinPlacement::new();
    for _ in 0..3 {
        for &cpu in &cpus {
            assert_eq!(placement.select(CpuMask::ALL, online), Some(cpu));
        }
    }
    let constrained = &cpus[cpus.len().saturating_sub(2)..];
    let affinity = constrained
        .iter()
        .fold(CpuMask::EMPTY, |mask, cpu| mask.with_cpu(*cpu));
    for _ in 0..3 {
        for &cpu in constrained {
            assert_eq!(placement.select(affinity, online), Some(cpu));
        }
    }
}

#[test]
fn constrained_and_rejected_placements_do_not_bias_unrestricted_work() {
    let cpus: Vec<_> = (0..hyper::cpu::MAX_CPUS.min(3))
        .map(|index| crate::require_some(CpuIndex::new(index)))
        .collect();
    let online = cpus
        .iter()
        .fold(CpuMask::EMPTY, |mask, cpu| mask.with_cpu(*cpu));
    let mut placement = RoundRobinPlacement::new();
    // Bootstrap admits only one CPU; later admission must work with the same policy.
    assert_eq!(
        placement.select(CpuMask::ALL, CpuMask::single(cpus[0])),
        Some(cpus[0])
    );
    for _ in 0..3 {
        for &cpu in &cpus {
            assert_eq!(placement.select(CpuMask::EMPTY, online), None);
            assert_eq!(placement.select(CpuMask::ALL, CpuMask::EMPTY), None);
            if let Some(&offline) = cpus.get(1) {
                assert_eq!(
                    placement.select(CpuMask::single(offline), CpuMask::single(cpus[0])),
                    None
                );
            }
            assert_eq!(
                placement.select(CpuMask::single(cpus[0]), online),
                Some(cpus[0])
            );
            assert_eq!(placement.select(CpuMask::ALL, online), Some(cpu));
        }
    }
}

#[test]
fn scheduling_profiles_encode_only_class_relevant_parameters() {
    assert_eq!(policy::PRIORITY_LEVELS, 256);
    let fifo = SchedulingPolicy::fifo(ThreadPriority::NORMAL);
    assert_eq!(fifo.class(), SchedulingClass::RealTime);
    assert_eq!(fifo.priority(), Some(ThreadPriority::NORMAL));
    assert_eq!(SchedulingPolicy::fair().class(), SchedulingClass::Fair);
    assert_eq!(SchedulingPolicy::fair().priority(), None);
    assert_eq!(SchedulingPolicy::Idle.class(), SchedulingClass::Idle);
    assert_eq!(SchedulingPolicy::Idle.priority(), None);
    assert!(ThreadPriority::HIGHEST < ThreadPriority::NORMAL);
    assert!(ThreadPriority::NORMAL < ThreadPriority::LOWEST);
    assert_eq!(ThreadPriority::new(7).get(), 7);

    let higher = SchedulingPolicy::fifo(ThreadPriority::new(40));
    let current = SchedulingPolicy::fifo(ThreadPriority::new(80));
    let equal = SchedulingPolicy::fifo(ThreadPriority::new(80));
    let lower = SchedulingPolicy::fifo(ThreadPriority::new(120));
    assert!(current.is_preempted_by(higher));
    assert!(!current.is_preempted_by(equal));
    assert!(!current.is_preempted_by(lower));
    assert!(SchedulingPolicy::fair().is_preempted_by(current));
    assert!(!current.is_preempted_by(SchedulingPolicy::fair()));
    assert!(!SchedulingPolicy::fair().is_preempted_by(SchedulingPolicy::fair()));
    assert!(SchedulingPolicy::Idle.is_preempted_by(current));
    assert!(!current.is_preempted_by(SchedulingPolicy::Idle));
}

#[test]
fn placement_requires_assignment_to_satisfy_affinity() {
    let cpu0 = crate::require_some(CpuIndex::new(0));
    let cpu1 = crate::require_some(CpuIndex::new(1));
    let affinity = CpuMask::EMPTY.with_cpu(cpu0).with_cpu(cpu1);
    let placement = crate::require_some(ThreadPlacement::new(
        cpu1,
        affinity,
        PlacementPolicy::Movable,
    ));
    assert_eq!(placement.assigned_cpu(), cpu1);
    assert!(placement.affinity().contains(cpu0));
    assert!(placement.affinity().contains(cpu1));
    assert_eq!(
        ThreadPlacement::new(cpu0, CpuMask::single(cpu1), PlacementPolicy::Movable),
        None
    );
    assert_eq!(
        ThreadPlacement::new(cpu0, affinity, PlacementPolicy::Pinned(cpu1)),
        None
    );

    let pinned = ThreadPlacement::pinned(cpu0);
    assert_eq!(pinned.assigned_cpu(), cpu0);
    assert!(pinned.affinity().contains(cpu0));
    assert!(!pinned.affinity().contains(cpu1));
    assert_eq!(pinned.policy(), PlacementPolicy::Pinned(cpu0));
    assert_eq!(pinned.last_cpu(), Some(cpu0));

    let movable = ThreadPlacement::movable(cpu1);
    assert_eq!(movable.policy(), PlacementPolicy::Movable);
    assert!(movable.affinity().contains(cpu0));
    assert!(movable.affinity().contains(cpu1));
    let running = crate::require_some(movable.mark_running(cpu1));
    assert_eq!(running.last_cpu(), Some(cpu1));
    assert_eq!(movable.mark_running(cpu0), None);

    let constrained_affinity = CpuMask::EMPTY.with_cpu(cpu0).with_cpu(cpu1);
    let constrained = crate::require_some(ThreadPlacement::movable_with_affinity(
        cpu1,
        constrained_affinity,
    ));
    assert_eq!(constrained.assigned_cpu(), cpu1);
    assert_eq!(constrained.affinity(), constrained_affinity);
    assert_eq!(constrained.policy(), PlacementPolicy::Movable);
    let narrowed = crate::require_some(constrained.with_affinity(CpuMask::single(cpu1)));
    assert_eq!(narrowed.assigned_cpu(), cpu1);
    assert_eq!(narrowed.affinity(), CpuMask::single(cpu1));
    assert_eq!(constrained.with_affinity(CpuMask::single(cpu0)), None);
    let migrated =
        crate::require_some(constrained.reassign_with_affinity(cpu0, CpuMask::single(cpu0)));
    assert_eq!(migrated.assigned_cpu(), cpu0);
    assert_eq!(migrated.affinity(), CpuMask::single(cpu0));
    assert_eq!(
        constrained.reassign_with_affinity(cpu1, CpuMask::single(cpu0)),
        None
    );
    assert_eq!(
        pinned.reassign_with_affinity(cpu0, CpuMask::single(cpu0)),
        None
    );
    assert_eq!(
        ThreadPlacement::movable_with_affinity(cpu1, CpuMask::single(cpu0)),
        None
    );

    assert!(CpuMask::EMPTY.is_empty());
    assert_eq!(
        constrained_affinity.without_cpu(cpu0),
        CpuMask::single(cpu1)
    );
}
