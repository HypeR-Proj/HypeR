// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn observation() -> MemoryObservation {
    MemoryObservation {
        captured_at_ns: 0,
        page_size: 4096,
        total_bytes: 100 * 4096,
        managed_bytes: 90 * 4096,
        free_bytes: 20 * 4096,
        kernel_bytes: 5 * 4096,
        slab_bytes: 10 * 4096,
        large_heap_bytes: 5 * 4096,
        page_table_bytes: 5 * 4096,
        vmo_bytes: 30 * 4096,
        guest_loaned_bytes: 20 * 4096,
        guest_private_bytes: 0,
        file_cache_bytes: 15 * 4096,
        slab_reclaimable_bytes: Some(2 * 4096),
        file_cache_reclaimable_bytes: Some(10 * 4096),
        ramfs_capacity_bytes: 1000,
    }
}

#[test]
fn summaries_do_not_double_count_shared_guest_pages_or_caches() {
    let memory = observation();
    let report = Summary::from_observation(&memory);
    assert_eq!(report.used, Some(68 * 4096));
    assert_eq!(report.available, Some(32 * 4096));
    assert_eq!(report.reclaimable, Some(12 * 4096));
    assert_eq!(report.reserved, 10 * 4096);
    assert_eq!(report.kernel, Some(23 * 4096));
    assert_eq!(report.buffered_cached, Some(17 * 4096));
    assert_eq!(report.user, 10 * 4096);
    assert_eq!(report.guest, 20 * 4096);
    assert_eq!(report.other, 0);
    assert_eq!(
        Some(report.allocated),
        report
            .kernel
            .zip(report.buffered_cached)
            .map(|(kernel, cache)| kernel + cache + report.user + report.guest + report.other)
    );
}

#[test]
fn unavailable_census_does_not_erase_physical_totals_or_mean_zero() {
    for (slab, file) in [(None, Some(4096)), (Some(4096), None), (None, None)] {
        let memory = MemoryObservation {
            slab_reclaimable_bytes: slab,
            file_cache_reclaimable_bytes: file,
            ..observation()
        };
        let report = Summary::from_observation(&memory);
        assert_eq!(report.reclaimable, None);
        assert_eq!(report.used, None);
        assert_eq!(report.available, None);
        assert_eq!(report.allocated, 70 * 4096);
        assert_eq!(report.kernel, slab.map(|bytes| 25 * 4096 - bytes));
        assert_eq!(report.buffered_cached, slab.map(|bytes| 15 * 4096 + bytes));
        assert_eq!(report.user, 10 * 4096);
        assert_eq!(report.guest, 20 * 4096);
        assert_eq!(report.other, 0);
    }
    let report = Summary::from_observation(&MemoryObservation {
        slab_reclaimable_bytes: Some(0),
        file_cache_reclaimable_bytes: Some(0),
        ..observation()
    });
    assert_eq!(report.used, Some(80 * 4096));
    assert_eq!(report.available, Some(20 * 4096));
    assert_eq!(report.kernel, Some(25 * 4096));
    assert_eq!(report.buffered_cached, Some(15 * 4096));
}

#[test]
fn reclaimable_slab_moves_between_kernel_and_cache_without_changing_allocations() {
    let memory = observation();
    for slab in [0, memory.slab_bytes] {
        let report = Summary::from_observation(&MemoryObservation {
            slab_reclaimable_bytes: Some(slab),
            ..memory
        });
        assert_eq!(report.kernel, Some(25 * 4096 - slab));
        assert_eq!(report.buffered_cached, Some(15 * 4096 + slab));
        // Pinned file pages stay in buffered/cached, but not in reclaimable.
        assert_eq!(report.reclaimable, Some(10 * 4096 + slab));
        assert_eq!(report.allocated, 70 * 4096);
    }
}
