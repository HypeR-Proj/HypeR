// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::sampling::{ProcessSample, Snapshot, ThreadSample};
use hyper_os::inspect::{CpuObservation, MemoryObservation};

pub fn snapshot(at: u64, runtime: u64, tid: u64) -> Snapshot {
    Snapshot {
        cpu: CpuObservation {
            captured_at_ns: at,
            online_cpus: 4,
            idle_ns: at * 3,
            user_thread_ns: at,
            kernel_thread_ns: 0,
            vcpu_ns: 0,
        },
        memory: MemoryObservation {
            captured_at_ns: at,
            page_size: 4096,
            total_bytes: 100_000_000,
            managed_bytes: 90_000_000,
            free_bytes: 40_000_000,
            kernel_bytes: 0,
            slab_bytes: 0,
            large_heap_bytes: 0,
            page_table_bytes: 0,
            vmo_bytes: 0,
            guest_loaned_bytes: 0,
            guest_private_bytes: 0,
            file_cache_bytes: 0,
            slab_reclaimable_bytes: Some(0),
            file_cache_reclaimable_bytes: Some(0),
            ramfs_capacity_bytes: 0,
        },
        processes: vec![ProcessSample {
            koid: 1,
            name: "worker".into(),
            state: "running",
            ..Default::default()
        }],
        threads: [(
            tid,
            ThreadSample {
                process_koid: 1,
                thread_koid: tid,
                runtime_ns: runtime,
                name: "thread".into(),
                role: "user",
                state: "resident",
            },
        )]
        .into(),
    }
}
