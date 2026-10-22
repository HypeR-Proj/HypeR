<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# `top`: live CPU and process activity

[top](../../../app/top/) samples CPU runtime counters, physical memory and per-process thread
runtime. Its CPU summary distinguishes Native user threads, kernel threads,
guest vCPUs and idle time. Use it to identify where CPU time is going while a
guest or Native command runs.

`top -d 0.5` refreshes every half second; `q` or Ctrl-C exits interactive mode.
`top -b -n 3` prints three snapshots without terminal control sequences or
keyboard input, suitable for capture in a file or pipeline. Process rows default
to descending CPU usage; `--sort name` or `--sort koid` changes the order.
`-p KOID[,KOID...]` and `--name TEXT` filter rows, and `--limit 10` bounds the
number displayed. CPU percentages use the total runtime across all host CPUs;
the summary remains system-wide even when process rows are filtered.
