// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capture into bounded stack records before allocating the report map. This
//! minimizes observer allocations between the first and last kernel samples.
use crate::snapshot::{
    Kind::{Counter, Gauge},
    Snapshot,
};
use hyper_os::inspect::{
    MemoryInspector,
    memory::{BUDDY_ORDERS, HEAP_CLASSES, PAGE_OWNERS},
};

pub fn capture(inspector: &MemoryInspector) -> hyper_os::Result<Snapshot> {
    let started = hyper_os::time::monotonic_now()?.as_nanoseconds();
    let physical = inspector.read()?;
    let heap = inspector.heap_statistics()?;
    let mut owners = [None; PAGE_OWNERS as usize];
    for (index, row) in owners.iter_mut().enumerate() {
        *row = Some(inspector.owner_statistics(index as u32)?);
    }
    let mut buddy = [None; BUDDY_ORDERS as usize];
    for (index, row) in buddy.iter_mut().enumerate() {
        *row = Some(inspector.buddy_statistics(index as u32)?);
    }
    let cache = inspector.file_cache_statistics()?;
    let mut classes = [None; HEAP_CLASSES as usize];
    if heap.profiling_enabled == 1 {
        for (index, row) in classes.iter_mut().enumerate() {
            let value = inspector.heap_class_statistics(index as u32)?;
            *row = Some(value);
        }
    }
    let finished = hyper_os::time::monotonic_now()?.as_nanoseconds();
    let mut sample = Snapshot::new(started, finished, heap.profiling_enabled == 1);
    macro_rules! metrics {
        ($source:ident, $prefix:expr, $kind:ident, [$($field:ident),+]) => {
            $(sample.add(format!("{}.{}", $prefix, stringify!($field)), $kind, $source.$field);)+
        };
    }
    metrics!(
        physical,
        "physical",
        Gauge,
        [
            page_size,
            total_bytes,
            managed_bytes,
            free_bytes,
            kernel_bytes,
            slab_bytes,
            large_heap_bytes,
            page_table_bytes,
            vmo_bytes,
            guest_loaned_bytes,
            guest_private_bytes,
            file_cache_bytes,
            ramfs_capacity_bytes
        ]
    );
    metrics!(
        heap,
        "heap",
        Gauge,
        [
            live_objects,
            live_slab_objects,
            live_large_objects,
            requested_bytes,
            peak_objects,
            peak_requested_bytes,
            cached_objects,
            cache_cpus,
            peak_allocated_pages
        ]
    );
    metrics!(
        heap,
        "heap",
        Counter,
        [
            allocations,
            failures,
            cache_hits,
            cache_misses,
            cache_refills,
            cache_drains,
            pressure_reclaims,
            reclaimed_objects,
            page_allocations,
            page_failures,
            page_deallocations
        ]
    );
    for (name, row) in ["kernel", "page_tables", "guest", "user", "file_cache"]
        .into_iter()
        .zip(owners.into_iter().flatten())
    {
        let prefix = format!("owner.{name}");
        metrics!(row, prefix, Gauge, [resident_pages, peak_pages]);
        metrics!(row, prefix, Counter, [allocations, failures, deallocations]);
    }
    for row in buddy.into_iter().flatten() {
        sample.add(
            format!("buddy.order_{}.free_blocks", row.order),
            Gauge,
            row.free_blocks,
        );
    }
    metrics!(
        cache,
        "file_cache",
        Gauge,
        [
            payload_pages,
            metadata_slab_pages,
            metadata_large_pages,
            index_capacity,
            clean_entries,
            loading_entries,
            live_payloads,
            metadata_budget_pages,
            table_present,
            admission_enabled,
            admission_paused,
            maintenance_pending,
            maintenance_running,
            growth_requested
        ]
    );
    for row in classes.into_iter().flatten() {
        let pool = if row.pool == 0 {
            "kernel"
        } else {
            "file_cache"
        };
        let size = if row.object_size == 0 {
            "large".to_owned()
        } else {
            row.object_size.to_string()
        };
        let prefix = format!("heap_class.{pool}.{size}");
        metrics!(
            row,
            prefix,
            Gauge,
            [pages, capacity, reserved, cached, live, requested_bytes]
        );
        metrics!(row, prefix, Counter, [allocations, deallocations]);
    }
    Ok(sample)
}
