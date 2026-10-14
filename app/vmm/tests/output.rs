// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use hyper_service::io::metrics::{AgentState, CpuUsage, GuestMetrics, MemoryUsage};

#[test]
fn guest_usage_distinguishes_available_memory_from_free_memory() {
    let mut bytes = Vec::new();
    write_guest_metrics(
        &mut bytes,
        GuestMetrics {
            state: AgentState::Ready,
            sample_age_ms: Some(12),
            cpu: Some(CpuUsage {
                busy_basis_points: 1234,
                iowait_basis_points: 56,
                interval_ms: 5000,
            }),
            memory: Some(MemoryUsage {
                total_bytes: 8192,
                available_bytes: 4096,
                free_bytes: 1024,
                buffers_bytes: 1024,
                cached_bytes: 2048,
                reclaimable_slab_bytes: 1024,
            }),
        },
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("12.34% busy; 0.56% I/O wait"));
    assert!(text.contains("4 KiB used / 8 KiB total; 4 KiB available; 1 KiB free"));
    assert!(text.contains("sample age 12 ms"));
}

#[test]
fn absent_agent_never_looks_like_zero_usage() {
    let mut bytes = Vec::new();
    write_guest_metrics(&mut bytes, GuestMetrics::default()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("unavailable"));
    assert!(!text.contains('%'));
    assert!(!text.contains("KiB"));
    let mut bytes = Vec::new();
    write_response(
        &mut bytes,
        Response::Entries {
            machines: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        String::from_utf8(bytes)
            .unwrap()
            .contains("no virtual machines")
    );
}
