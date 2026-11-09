<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Storage performance history

[Documentation](../README.md) · [Project status](../project/status.md) ·
[Storage test procedure](https://github.com/HypeR-Proj/system-tests/blob/main/hardware/storage/README.md)

This page tracks physical Raspberry Pi 5 storage measurements across HypeR
versions, with historical Raspbian and Alpine references. Append new results;
keep previous measurements and identify changes in workload or synchronization
scope. Last record added: **2026-11-08**.

## Measurement contract

- Sequential tests write **1 GiB** in **128 KiB** requests. Reported throughput
  is 1024 MiB divided by write-plus-final-volume-sync time. Payload generation
  is inside the write interval; full readback verification is outside it.
- A sync4k sample includes a **4 KiB write plus synchronization**, at application
  queue depth one. Each successful command contains 128 samples. Percentiles
  use nearest rank; IOPS is 128 divided by the sum of sample durations.
- **File sync and volume sync are separate workloads.** Legacy `write`,
  `overwrite` and `sync4k` records below used volume sync, established by the
  historical workload and report. Format 2 explicitly encodes the scope.
- These are single-run development observations unless explicitly stated.
  Card identity/state, free space, filesystem/mount settings, temperature,
  clock state and background activity were not consistently recorded. Raspbian
  used a separate card. Cross-system numbers describe complete deployments,
  not isolated hypervisor overhead or a controlled filesystem comparison.
- Readback may hit caches. These tests do not measure cold-read throughput or
  prove power-loss durability. Trace captures such as run 501 are excluded
  from these throughput tables; profiling changes the workload.
- Dates identify the historical survey or delivery/report session, not a
  recovered timestamp for every command. Unknown metadata stays unknown.

## Sequential write history

All rows include final **volume sync**. All retained completed bulk runs passed
full 1 GiB readback. The Alpine row is imported from the historical published
summary; its original log is not included here.

| Session | Filesystem | New-file MiB/s | New-file total s | Overwrite MiB/s | Overwrite total s |
| --- | --- | ---: | ---: | ---: | ---: |
| [2026-10-06 Raspbian reference](data/2026-10-06-raspbian.txt) | Not recorded | 24.76 | 41.36 | 25.48 | 40.19 |
| 2026-10-18 Alpine guest reference | ext4 | 23.03 | 44.46 | 22.81 | 44.89 |
| [2026-10-23 Native FAT](data/2026-10-23-native-fat.txt) | FAT32 | 9.99 | 102.48 | 17.80 | 57.53 |
| [2026-11-05 Native F2FS](data/2026-11-05-native-f2fs.txt) | F2FS | 15.91 | 64.36 | 15.98 | 64.07 |
| [2026-11-05 F2FS tuning (201/202)](data/2026-11-05-f2fs-tuning.txt) | F2FS | 17.20 | 59.54 | 15.84 | 64.64 |
| [2026-11-07 F2FS pipeline (401/402)](data/2026-11-07-f2fs-pipeline.txt) | F2FS | 18.37 | 55.73 | 16.12 | 63.50 |
| [2026-11-08 F2FS pipeline (601/602)](data/2026-11-08-f2fs-pipeline.txt) | F2FS | 20.06 | 51.05 | 18.24 | 56.15 |

The November 8 measurements versus November 7 show **9.17%** higher new-file
throughput and **13.10%** higher overwrite throughput. Final volume-sync times
changed only from 451.851 to 447.347 ms and 730.791 to 734.843 ms, respectively.
The observed gain is in the bulk write phase; this is not a repeated A/B estimate
or attribution to an individual optimization.

## 4 KiB write plus volume sync

Each row has 128 samples. Historical Raspbian filesystem details were not
recorded; do not silently label it ext4.

| Session | Mean ms | P50 ms | P95 ms | P99 ms | Maximum ms | IOPS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2026-10-06 Raspbian reference | 5.697 | 4.897 | 12.998 | 32.597 | 52.624 | 175.54 |
| 2026-10-18 Alpine guest reference | 14.10 | — | 20.94 | 62.96 | 392.00 | 70.93 |
| 2026-10-23 Native FAT | 7.699 | 7.520 | 9.464 | 21.414 | 63.737 | 129.88 |
| 2026-11-05 Native F2FS | 40.559 | 26.637 | 65.359 | 67.547 | 71.521 | 24.66 |
| 2026-11-05 F2FS tuning (201/202) | 25.361 | 24.128 | 31.907 | 50.261 | 71.040 | 39.43 |

## 4 KiB write plus file sync

These results must not be compared to the volume-sync table as equivalent work.
The November 7 run has no valid file-sync measurement.

| Session | Mean ms | P50 ms | P95 ms | P99 ms | Maximum ms | IOPS |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2026-11-05 F2FS tuning (201/202) | 18.422 | 18.076 | 20.787 | 22.150 | 45.479 | 54.28 |
| 2026-11-08 F2FS pipeline (601/602) | 18.713 | 18.511 | 20.617 | 27.607 | 29.499 | 53.44 |

The November 8 mean is close to November 5 (18.713 versus 18.422 ms), while
P99 is higher (27.607 versus 22.150 ms) and the maximum is lower (29.499 versus
45.479 ms). With 128 samples per session and uncontrolled card state, this does
not establish a tail-latency improvement or regression.

### Excluded attempts

`sync4k-file 403` after overwrite 402 and `sync4k-file 603` after overwrite 602
failed at offset zero during initial verification, before timed writes. The
supplied instructions incorrectly incremented `RUN_ID`, which is also the
payload seed. These are command errors, not filesystem-corruption evidence.
They remain in the retained records but are excluded from statistics.
The corrected `sync4k-file 602` passed initial verification and all 128 samples.

For an existing file, `sync4k-*` and `check` must reuse the latest writer's seed:

```sh
/bin/storage-qual write-volume /data/storage-qual.bin 601
/bin/storage-qual overwrite-volume /data/storage-qual.bin 602
/bin/storage-qual sync4k-file /data/storage-qual.bin 602
```

## Build identity and evidence

[Record index](data/README.md) lists the retained output, extracted statistics
and build metadata. Logs contain the STORAGE lines in their original order,
with terminal padding removed. November 7 is explicitly a transcription from
the supplied message. No missing serial output is reconstructed as raw evidence.

| Session | Source identity / known environment |
| --- | --- |
| Raspbian, October 6 | Native OS on a separate SD card; exact OS revision, filesystem and mount options not recorded here. |
| Alpine, October 18 | Image `0.1.0-7e28369b46d9`; ext4, two vCPUs / 256 MiB; I/O VM `56b1b8788820`, one vCPU / 64 MiB. Historical published summary only. |
| Native FAT, October 23 | Kernel `3456aa050270`, built `2026-10-23T13:06:05Z`; I/O VM `56b1b8788820`, one vCPU / 64 MiB; Alpine stopped. |
| Native F2FS, November 5 | Kernel `a06c96301ecf`, built `2026-11-04T17:27:12Z`; pre-file-sync-integration workload. |
| F2FS tuning, November 5 | Kernel base `987875d07c44` **plus uncommitted changes** at measurement; see image SHA-256 and source inventory. |
| F2FS pipeline, November 7 | Kernel base `6192779a8140` **plus uncommitted changes** at measurement; see image SHA-256 and source inventory. |
| F2FS pipeline, November 8 | Kernel `d5cbaa37c76af20d5bfed4b2be95c9f83b0e8e84`; clean inventoried sources; tracing/profiling disabled; 32 KiB kernel stacks; 2 GiB F2FS data partition. |

An image built from a dirty source tree is identified by its artifact hash;
the recorded base commit alone cannot reproduce it. The compact build records
retain component revisions, dirty flags and workload hashes, not the dirty
patches or complete build inputs. Disk images and Perfetto traces are not stored
in this Git repository, and no public artifact download is claimed for these
locally delivered images.

The earlier
[published survey](https://github.com/HypeR-Proj/HypeR/blob/8d42d047058a2b58a291803700e86487fb723f7a/README.md#exploratory-pi-5-io-measurements)
provides the Alpine reference and original environment notes. It also records
the October 21 Native FAT baseline: 9.11 MiB/s new-file write (one run),
14.88 MiB/s overwrite (median of three), and 8.73 ms mean / 16.59 ms P99 for
write-plus-volume-sync. Those aggregation rules differ from the single-run rows
above, so that baseline is retained as a separate historical note.

## Adding a measurement

Append the results and keep the original records, including failed attempts.
Record the source revisions and dirty state, image and I/O VM hashes, board,
card, filesystem/mount configuration, free space, tracing state, workload version,
sync scope, repetition count and run seed when available. Mark missing values
explicitly. Preserve integer nanoseconds in the data; round only the tables.
Use the storage tool's nearest-rank calculation. A run ID is not a unique
measurement key: the same seed is intentionally reused by different commands.

Store compact serial records and metadata alongside this page. Link large images
or traces to release/artifact storage when published, with checksums. Keep the
[test implementation and procedure](https://github.com/HypeR-Proj/system-tests/blob/main/hardware/storage/README.md)
in the system-tests repository. QEMU correctness checks are not physical-board
performance measurements.
