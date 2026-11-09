<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Storage measurement records

[Performance history and interpretation](../storage.md)

`storage-results.json` retains integer-nanosecond records, derived statistics,
completion state and exclusions. Each text file contains extracted STORAGE lines
with terminal padding removed, rather than a complete boot log. November 7 is a
transcription from a user message. Files are licensed under this repository's
Apache-2.0 license. SHA-256 values in the index cover the text files as stored.

- **2026-10-06 Raspbian reference**: [records](2026-10-06-raspbian.txt)
- **2026-10-23 Native FAT**: [records](2026-10-23-native-fat.txt)
- **2026-11-05 Native F2FS**: [records](2026-11-05-native-f2fs.txt), [build identity](2026-11-05-native-f2fs-build.json)
- **2026-11-05 F2FS tuning (201/202)**: [records](2026-11-05-f2fs-tuning.txt), [build identity](2026-11-05-f2fs-tuning-build.json)
- **2026-11-07 F2FS pipeline (401/402)**: [records](2026-11-07-f2fs-pipeline.txt), [build identity](2026-11-07-f2fs-pipeline-build.json)
- **2026-11-08 F2FS pipeline (601/602)**: [records](2026-11-08-f2fs-pipeline.txt), [build identity](2026-11-08-f2fs-pipeline-build.json)

[Machine-readable results](storage-results.json) use capture ID plus command
order to identify measurements, not RUN_ID alone. Failed attempts are retained
with `complete: false` and no performance statistics. Legacy sync scope is
annotated from the known historical workload, not inferred from modern defaults.

Build identities are compact extracts of the original image build metadata;
they omit local absolute paths and compiler invocations. They include source
inventory and artifact hashes but do not replace source patches for dirty builds.
The Alpine row in the performance page comes only from the linked historical
published summary and has no reconstructed log in this directory.
