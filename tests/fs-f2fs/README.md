<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Disposable F2FS integration fixture

Build the kernel, installed SDK, bootstrap and formal board artifacts from the
same checkout. Mixing ABI versions gives misleading startup failures. Build the
[experimental core](../../lib/fs-f2fs/README.md) and worker first. Then build the
standalone Native probe through the installed SDK:

```sh
HYPER_SYSROOT="$PWD/target/sdk/aarch64" \
  target/sdk/aarch64/bin/hyper-cargo build \
  --manifest-path tests/native/f2fs-smoke/Cargo.toml --release --offline
```

Create a **new, separate** fixture directory using a prepared board's GPT disk,
bootstrap archive and `system-root`. Supply the actual worker and probe artifacts
from the preceding build (Cargo output directories depend on build invocation).

```sh
python3 tests/fs-f2fs/prepare.py \
  --base target/board/qemu --output target/f2fs/acceptance \
  --worker app/target/aarch64-unknown-hyper/release/hyper-fs-f2fs-system \
  --probe tests/native/f2fs-smoke/target/aarch64-unknown-hyper/release/hyper-f2fs-smoke \
  --library target/f2fs/host/libhyper-f2fs.so
python3 tests/qemu/verify-f2fs.py \
  --image kernel/target/aarch64-unknown-none-softfloat/kernel/hyper.img \
  --initramfs target/f2fs/acceptance/bootstrap.cpio \
  --disk target/f2fs/acceptance/disk.img \
  --board target/f2fs/acceptance/deployment.json \
  --log target/f2fs/acceptance/run.log
```

Preparation refuses an existing output directory, validates both GPT copies,
clones the source, writes freshly formatted F2FS only to the clone's data extent,
and refreshes the clone's EROFS system payload. It never modifies the source
board. `deployment.json` retains source partition geometry, including its original
FAT formatter declaration; `fixture.json` records the F2FS override. Do not use
the ordinary full-board formatter on this fixture to preserve F2FS contents.

The acceptance test writes and synchronizes 4 MiB through ordinary Native file
APIs, renames it, creates 80 directory entries, enumerates them, unmounts, stops
QEMU, and starts a new instance. The second instance verifies every byte,
checks truncate/regrow zeroing and nested directory creation, removes the test tree and unmounts again.
A third instance writes and verifies 12 MiB, crossing the 8 MiB background
writeback threshold before explicit sync.
`/bin/f2fs-smoke ram` independently tests the C++/Rust runtime against an owned
RAM device; it never formats a block capability. A failed test retains its logs
and disposable disk for inspection.

## Initial integration result (2026-10-29)

AArch64 QEMU with the resident I/O VM passed both phases above, including a full
QEMU restart and recursive removal through Rust std. The RAM-core probe and a 12 MiB writeback-threshold workload also
passed on Native. The latter submitted 41 writes (12,701,696 bytes including
metadata), with four writes outstanding at peak. RISC-V cross-compilation passed. Host tests passed malformed geometry/extension rejection and
persistent write/flush failure propagation, including refusing a later sync
without remount after the device recovers.

With `io-statistics`, the write phase including final unmount reported:

| Measurement | Result |
| --- | ---: |
| Data file payload | 4,194,304 bytes |
| Additional small files | 80 |
| Native queue writes | 23 on a fresh image; 24 on a reused image |
| Bytes submitted for writing, including metadata | 4,943,872 |
| Native queue flushes | 6 |
| Peak outstanding writes | 4 |

These are submissions at the Native block queue boundary, not physical device
commands or an end-to-end throughput benchmark. They demonstrate aggregation and
multiple outstanding writes across the I/O VM path. The host test additionally
observed a 1 MiB merged operation before the Native adapter's 512 KiB split.
The verify/delete/unmount phase used 12 writes, 73,728 written bytes and six
flushes. No FAT performance comparison or hardware latency claim is implied.

Integration also exercised and fixed the public Rust std recursive-removal
path: revalidate directory identity while an opened handle pins it, then release
the traversal handle before conditional removal on remote volumes.
