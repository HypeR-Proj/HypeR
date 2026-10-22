<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Memory accounting with free

## Memory cache reporting

The kernel observation exposes raw physical allocations, without choosing the
`free` presentation policy: total RAM, allocator-managed RAM, free pages,
direct kernel pages, slab backing, large heap backing, page tables, VMO backing,
private guest backing, and file-cache backing. The physical allocation
categories are disjoint and captured under the allocator lock. Consumers derive
reserved RAM as `total - managed` and allocated RAM as `managed - free`.

`free` computes its main row in userspace:

- `reclaimable = file-reclaimable + slab-reclaimable`;
- `available = free + reclaimable`;
- `used = total - available`, including reserved RAM;
- consequently `total = used + free + reclaimable`.

`available` estimates recoverable space, without reserving pages or promising
contiguous allocations. File and slab censuses have independent validity. If
either cannot complete, the dependent main-row estimates show `—`. If the slab
census is unavailable, `kernel` and `buffered/cached` also show `—`, since their
boundary depends on reclaimable slab. Total, free, reserved, allocated, user,
guest and other remain available. A successful empty census is zero. Cache
reader pins can change after any observation.

The `Owners` row partitions managed allocations without double counting:

- `kernel = direct kernel pages + slab + large heap + page tables - slab-reclaimable`;
- `buffered/cached = file-cache backing + slab-reclaimable`;
- `user` and `guest` separate Native and guest backing as described below;
- `other` covers any allocated pages outside the raw ownership counters.

`kernel` represents allocations not eligible for cache reclamation at sampling
time; it does not imply permanent allocation. Kernel stacks, heap objects and
page tables can still be released when their owners exit or drop them. The
kernel image and early boot reservations remain in `reserved`.

VMO backing retained by an exclusive guest-memory lease is counted once in
`guest`, even when a backend aliases it or the lease is cloned. It is removed
from `user`; that field describes remaining VMO backing, not summed process
RSS. Guest accounting includes loaned transport buffers, not just advertised
RAM. Sparse uncommitted guest RAM counts as zero. Final lease release removes
guest use even if Native handles still retain the underlying VMO pages.

`Cache` separates unpinned clean file pages and shrinker-managed index/header
backing from complete kernel slab pages recoverable by draining CPU-local
magazines. File-cache ownership also includes loading or
pinned pages, which cannot immediately be reclaimed. A slab page is recoverable
only if all its outstanding central reservations are cached tokens. The bounded
census validates magazine epochs and never drains caches to measure them.
Neither cache detail is an additional physical pool to add to `Owners`.
The `non-reclaimable` cache detail reports file-cache backing minus reclaimable
file backing. It represents pinned/loading payloads, and is already part of
both `buffered/cached` and `used`. Thus the visible
breakdown reconciles as `used = reserved + kernel + user + guest + cache
non-reclaimable + other`. Use `--bytes` for exact arithmetic: human-readable
fields are independently rounded down.

`Ramfs` shows a logical capacity metric within `kernel`. It sums writable ramfs
payload-capacity reservations, including transient replacement reservations.
It excludes allocator rounding, directory metadata, and borrowed boot-archive
contents. It overlaps heap backing and must not be added to physical totals.
Ramfs data is authoritative and cannot be discarded as cache. Borrowed
boot-archive storage remains in reserved RAM. File-cache index arrays and page
descriptors use a separate heap pool; their
physical backing is included in `buffered/cached`, not `kernel`. Cache totals
include mapped or pinned data and metadata. `file-reclaimable` includes
shrinker-managed metadata and is an estimate, not a promise of immediate
recovery: live pins can retain a minimum index and partially occupied header
slabs. Growing a disposable cache must not be reported as permanent `used`
memory merely because its bookkeeping also consumes RAM.

`top` reports total, allocated (including reserved), free and reserved bytes;
its allocated count includes reclaimable caches. Default units retain small
KiB quantities; `free --bytes` avoids rounding.


## Sampling and units

`free -b` prints exact bytes; `-k`, `-m` and `-g` select whole KiB, MiB and GiB.
`free -m -c 5 -s 0.5` collects five snapshots half a second apart, starting
immediately. Sampling is finite and defaults to one snapshot; `-s` alone does
not request continuous monitoring.
