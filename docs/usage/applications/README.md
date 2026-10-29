<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native applications

Formal commands are installed under `/bin` on the independent system root.
Run `COMMAND --help` for command options; `echo` treats unknown options as text.
The shell also has builtins, including `echo` and `printf`.

| Task | Manual |
| --- | --- |
| Interactive commands, history, completion, scripts and redirection | [Shell](https://hyper-proj.github.io/shell/docs/usage.html) |
| Edit a text file | [editor](https://hyper-proj.github.io/editor/docs/usage.html) |
| Page through files or pipeline output | [less](https://hyper-proj.github.io/less/docs/usage.html) |
| List, copy, rename, remove and search files | [File tools](https://hyper-proj.github.io/coreutils/docs/files.html) |
| Mount and unmount authorized volumes | [mount and umount](https://hyper-proj.github.io/mount/docs/usage.html) |
| Inspect processes and threads | [ps](https://hyper-proj.github.io/ps/docs/usage.html) |
| Watch CPU and process activity | [top](https://hyper-proj.github.io/top/docs/usage.html) |
| Read physical memory totals and cache accounting | [free](https://hyper-proj.github.io/free/docs/usage.html) |
| Compare allocation/cache snapshots | [memstat](https://hyper-proj.github.io/kernel/docs/usage/memory-diagnostics.html) |
| Sample CPU activity or record kernel event timelines | [perf / flame graphs](https://hyper-proj.github.io/perf/docs/profiling.html), [perf trace / Perfetto](https://hyper-proj.github.io/perf/docs/tracing.html) |
| Inspect objects, handles and permissions | [handle](https://hyper-proj.github.io/handle/docs/usage.html) |
| Inspect executable/shared-library dependencies | [ldd](https://hyper-proj.github.io/ldd/docs/usage.html) |
| Start, stop, inspect or attach to a VM | [vmm](https://hyper-proj.github.io/vmm/docs/usage.html) |
| Query service state, control components or read logs | [componentctl and journalctl](https://hyper-proj.github.io/component-manager/docs/services.html) |

[Application I/O and authority](https://hyper-proj.github.io/lib-dynamic-loader/docs/execution.html) explains terminal ownership,
standard streams and which diagnostic capabilities a launched command receives.
[Native application development](https://github.com/HypeR-Proj/build/blob/main/app/README.md) covers the SDK and build
workspace; [component design](https://hyper-proj.github.io/component-manager/docs/components.html) covers background
service configuration and lifecycle.

Bootstrap recovery is separate: stage-1 uses `/svc/fallback-sh` before ownership
transfer, while formal interactive sessions use `/bin/sh`. The old archive is
visible under `/boot` only until retirement. See
[boot and system layout](https://hyper-proj.github.io/bootstrap/docs/boot-layout.html).
