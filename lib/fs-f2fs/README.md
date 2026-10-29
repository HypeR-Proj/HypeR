<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Experimental Fuchsia F2FS adapter

`hyper-fs-f2fs` wraps the pinned Fuchsia C++ media implementation with an owned
Rust FFI boundary. `svc/fs-f2fs` implements the existing Native filesystem worker
protocol. The kernel has no C++ or F2FS dependency. The worker is disabled by
default (`SYSTEM_APP_FS_F2FS_SYSTEM=n`); ordinary persistent images remain FAT.

The C++ core retains Fuchsia's inode, directory, NAT/SIT, segment allocation,
checkpoint, garbage collection and writeback code. The patch removes Zircon
VFS, pager, component, Inspect and memory-pressure dependencies. Explicit file
reads and writes use the core's page/node cache and block allocation routines.
The private page backing is heap memory, not a Zircon VMO emulation service.

Rust owns the device for the entire mount. Public operations require exclusive
access. Background C++ callbacks borrow a stable mutex-protected device; teardown
joins the executor before releasing that device. A successful `close` flushes
and checkpoints; dropping an unsuccessful mount abandons it without claiming
successful synchronization.

## I/O and durability

The Native adapter uses the existing `BlockQueue`, including its admission and
backpressure rules. Each submission is at most 512 KiB, with at most four writes
outstanding. Writeback merges adjacent filesystem blocks before submission.
The callback drains every accepted write before returning, including after a
later admission failure. No source pointer survives a Native submission.

A preflush completes before checkpoint commit I/O. A FUA operation writes its
payload, joins it, and completes an explicit queue flush before returning.
Normal `sync` flushes payload before publishing a live checkpoint; unmount writes
a clean shutdown checkpoint. Any failed write sets the core's checkpoint error
flag and fails synchronization instead of retrying data writes indefinitely.
Successful page-cache writes alone do not imply durability.

The dirty-page threshold is 2,048 pages (8 MiB), using upstream background
writeback. This is not a hard total-memory quota: inode, directory and clean-page
caches have additional costs. The experimental worker retains the ordinary
128 MiB committed-page ceiling and allows 1,024 mappings/objects for its heap
regions and staging pools. Exceeding those quotas currently fails the process. Reads do not export physical extents because that
would bypass dirty data held by the media implementation.

## Fit for HypeR

The initial Native run demonstrates the property missing from a purely serial
sector writer: four outstanding queue writes and aggregation of a 4 MiB payload
plus 80 small files into 23 write submissions. See the [test result](../../tests/fs-f2fs/README.md).
This makes the port a plausible writeback experiment for the I/O VM transport.

The remaining performance risks are substantial: both kernel and C++ caches can
retain file data; payload still crosses cache, staging and copied queue buffers;
GC and checkpoint latency have not been characterized under sustained pressure.
The port also introduces a C++ runtime and replaces Fuchsia's pager with explicit
heap-backed pages. It is appropriate for controlled comparison, while replacing
production persistence requires those memory, GC and crash-recovery tests first.

## Supported contract and limits

The experimental worker supports regular files, directories, paginated listing,
read/write, rename, removal, truncate, timestamps, sync and orderly handoff.
New nodes use mode `0777`, matching the current FAT worker's writable Native
mode projection. Read-only mounts, symlinks, hard links, extended attributes,
compression, encryption and discard are not exposed by this adapter.

Use images produced by the pinned formatter below. The upstream superblock
structure ends at `cp_payload`; this adapter rejects nonzero trailing superblock
fields rather than guessing support for modern F2FS extensions. It also checks
that the media geometry covers the recorded block count. This is deliberately
narrower than accepting arbitrary Linux `mkfs.f2fs` output. Times before the Unix
epoch or beyond the signed nanosecond range are rejected.

This is a disposable-media integration, not a qualified replacement for `/data`.
Long-running GC/full-disk workloads, exhaustive allocation and I/O fault injection,
unclean shutdown/power-loss recovery, Linux interoperability, hard cache limits,
and real SD/NVMe performance still need qualification. QEMU timing is not a
hardware performance result.

## Sources and licenses

Fuchsia is pinned to
[`fff7033e9cc337f2ab4fe8629a61c0c66bdb5cff`](https://fuchsia.googlesource.com/fuchsia/+/fff7033e9cc337f2ab4fe8629a61c0c66bdb5cff/src/storage/f2fs/).
`sources.json` pins each archive by SHA-256. `fuchsia.patch` records changes to
upstream files; `cpp/` contains new platform adapters. Builds apply these to a
private output tree. Ordinary Cargo builds never download sources. The repository Rustdoc driver
sets `HYPER_DOCUMENTATION=1` to document the Rust interface without compiling or
linking media code; executable builds must leave that switch unset.

The media core uses the Fuchsia BSD license; ext2 hashing retains its FreeBSD
BSD-2-Clause notice, and safemath retains its BSD notice. The Native C++ support
uses LLVM libc++ 21.1.8 (Apache-2.0 with LLVM exception) and declarations from
musl 1.2.5 (MIT). No musl implementation or Linux filesystem code is linked.
Complete notices are retained beside this file and under `licenses/`, and are
included under `share/licenses/fs-f2fs` in system images. Google's upstream
[PATENTS grant](FUCHSIA-PATENTS) is preserved; it is not a general clearance of
third-party F2FS patent claims.

## Build and test

Start with an assembled SDK and the Clang/LLVM tools used by the Native build.
`HYPER_CXX`, `HYPER_CLANG` and `LLVM_AR` can select explicit tool paths.

```sh
make sdk ARCH=aarch64
python3 lib/fs-f2fs/build.py --sdk target/sdk/aarch64 \
  --output target/f2fs/aarch64 --fetch
python3 lib/fs-f2fs/build.py --host --output target/f2fs/host
python3 tests/fs-f2fs/host.py target/f2fs/host/libhyper-f2fs.so
python3 lib/fs-f2fs/host.py --library target/f2fs/host/libhyper-f2fs.so \
  --output target/f2fs/new-volume.img --size-mib 128
```

The formatter exclusively creates a new image; it never overwrites an existing
file. The host test exercises real media algorithms, including dirty reads,
checkpoint/remount, overwrite, truncate/regrow, sparse data and directory paging.

Enable `SYSTEM_APP_FS_F2FS_SYSTEM` in an alternate product configuration to deploy
`/svc/fs-f2fs`. Both mount managers recognize `f2fs`, but bootstrap never links
this formal implementation. To build it independently, use installed
`hyper-cargo` with `HYPER_SYSROOT` set and select `hyper-fs-f2fs-system` in
`app/Cargo.toml`. The optional `io-statistics` feature reports queue counts at
unmount for test evidence.

`tests/native/f2fs-smoke` is an installed-SDK consumer. Its `ram` mode formats only
an owned RAM device. Its `write ROOT` and `verify ROOT` modes exercise the ordinary
Native VFS on a prepared disposable mount. `tests/qemu/verify-f2fs.py` runs those
modes, plus a 12 MiB background-writeback workload, in separate QEMU instances and explicitly unmounts the volume in each run.
See [the fixture instructions](../../tests/fs-f2fs/README.md).
