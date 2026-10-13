// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace interpretation of raw physical counters. No display policy lives
//! in the kernel: reclaimability is an estimate, not an allocation guarantee.

use hyper_os::inspect::MemoryObservation;

pub struct Summary {
    pub reserved: u64,
    pub allocated: u64,
    pub reclaimable: Option<u64>,
    pub available: Option<u64>,
    pub used: Option<u64>,
    pub kernel: Option<u64>,
    pub buffered_cached: Option<u64>,
    pub user: u64,
    pub guest: u64,
    pub other: u64,
}

impl Summary {
    pub fn from_observation(memory: &MemoryObservation) -> Self {
        // MemoryInspector validates the physical identities and subset bounds.
        // Reserve is unavailable physical RAM, so it remains in used, while
        // reclaimable pages are shown separately from non-reclaimable use.
        let reserved = memory.total_bytes - memory.managed_bytes;
        let allocated = memory.managed_bytes - memory.free_bytes;
        let reclaimable = memory
            .slab_reclaimable_bytes
            .zip(memory.file_cache_reclaimable_bytes)
            .map(|(slab, file)| slab + file);
        let available = reclaimable.map(|bytes| memory.free_bytes + bytes);
        let used = available.map(|bytes| memory.total_bytes - bytes);
        let kernel_backing = memory.kernel_bytes
            + memory.slab_bytes
            + memory.large_heap_bytes
            + memory.page_table_bytes;
        // Move recoverable slab backing into the cache bucket, keeping the
        // physical partition disjoint and kernel limited to unreclaimable use.
        let kernel = memory
            .slab_reclaimable_bytes
            .map(|slab| kernel_backing - slab);
        let buffered_cached = memory
            .slab_reclaimable_bytes
            .map(|slab| memory.file_cache_bytes + slab);
        let user = memory.vmo_bytes - memory.guest_loaned_bytes;
        let guest = memory.guest_private_bytes + memory.guest_loaned_bytes;
        let attributed = kernel_backing
            + memory.vmo_bytes
            + memory.guest_private_bytes
            + memory.file_cache_bytes;
        Self {
            reserved,
            allocated,
            reclaimable,
            available,
            used,
            kernel,
            buffered_cached,
            user,
            guest,
            other: allocated - attributed,
        }
    }
}

#[cfg(test)]
#[path = "../tests/report.rs"]
mod tests;
