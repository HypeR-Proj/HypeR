<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# `top`: live CPU and process activity

[top](https://github.com/HypeR-Proj/top/tree/main/) monitors CPU time, physical memory, processes and threads.
The **first snapshot appears immediately**; `-d` controls the interval between
subsequent samples. `top -b -n 1 -d 60` therefore prints once and exits without
waiting 60 seconds. The first frame is labelled `since boot`: CPU categories use
cumulative counters and task percentages use observed lifetime runtime divided
by monotonic uptime. Later frames are labelled `interval` and use counter deltas.

```sh
top -d 0.5
top -H --sort time
top -b -n 3 -d 1 --normalize
top -b -n 1 -p 0x100000001 --limit 10
```

`-b` prints plain snapshots without keyboard input or terminal escape sequences;
redirected stdout also selects this mode. `-n` counts samples, including the first,
not interface redraws. Interactive mode clips to the terminal size, handles resize
notifications, and restores its fullscreen presentation and settings on exit.

## Rows, filters and sorting

Process rows show the full generation-bearing KOID, `%CPU`, `TIME+`, observed
thread count, lifecycle state and name. `-H` shows individual threads, their own
KOIDs, roles, registry phases and owning processes. Unfiltered thread mode also
includes kernel, idle and guest-vCPU threads.

`-p KOID[,KOID...]` and `--name TEXT` select processes; in thread mode they select
threads by their owning process, excluding unowned kernel threads. `-l` / `--limit`
bounds matching rows. Summaries always describe the whole inspector scope.

`-o` / `--sort` accepts `cpu` (default), `time`, `threads`, `name` or `koid`.
CPU/time/thread-count sorts descend; name/KOID sorts ascend, with KOID resolving
ties. `-R` / `--reverse` reverses the order. `-i` / `--idle-hide` hides rows with
zero sampled CPU usage. An empty result keeps its summary and column headings.

## Interactive controls

These common controls follow [Linux top](https://man7.org/linux/man-pages/man1/top.1.html)
where the underlying metrics exist; `C` is HypeR's name-sort shortcut.

| Key | Action |
| --- | --- |
| `q`, Ctrl-C | Quit |
| Space, Enter | Sample immediately |
| `d`, `s` | Enter a new delay, from 0.1 to 60 seconds; Escape cancels |
| `H` | Toggle process/thread rows |
| `I` | Toggle per-CPU / whole-machine task CPU percentages |
| `P`, `T`, `N`, `C` | Sort by CPU, time, KOID, name |
| `R` | Reverse the selected order |
| `i` | Toggle zero-activity rows |
| Up/Down, PageUp/PageDown | Scroll rows |
| Left/Right | Scroll table columns |
| Home, End | Reset scroll / show the last page |
| `h`, `?` | Show help; another key returns |

Sorting, scrolling, help and view changes redraw the retained snapshot without
resetting the sampling interval. Unknown keys do not force a refresh. Delay
changes take an immediate new sample and schedule the next one at the new period.

## Accounting and available metrics

Task `%CPU` defaults to **100% for one busy CPU**, so a multithreaded process can
exceed 100%. `--normalize` or `I` instead divides by aggregate host CPU runtime,
where 100% represents the whole machine. The CPU summary always divides its
user-thread, kernel-thread, vCPU and idle categories by aggregate runtime.
Counters use elapsed monotonic nanoseconds rather than periodic scheduling ticks.

Thread `TIME+` is its cumulative runtime in minutes, seconds and hundredths.
Process `TIME+` sums observed thread runtimes and retains time from threads that
retire while this monitor runs. Threads already gone before the first scan, or
created and destroyed entirely between scans, are not observable. Newly observed
threads have no prior interval baseline, so interval `%CPU` begins at their next
sample; their cumulative time is available immediately. Scans are weakly
consistent, and thread samples are joined by complete KOIDs, never recycled slots.

`STATE` is the Native process lifecycle (for example `running` or `stopped`),
not Linux scheduler `R/S/D` state. Thread `REGISTRY` reports `resident` or
`retiring`; resident does not imply running. Memory is physical system-wide
allocation, including reserved memory, plus file-cache backing.

The current inspector interfaces do not expose per-process RSS/VIRT/%MEM,
per-CPU runtime counters, scheduler load averages or user/nice fields. Those
columns and their associated sort/actions are not offered. Observed KOIDs also
do not grant stop/signal authority, so this monitor has no kill or renice action.
