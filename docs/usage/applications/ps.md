<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# `ps`: process and thread inventory

[ps](../../../app/ps/) lists visible Native processes with their KOIDs, immutable service
names, lifecycle states and active/pending thread counts. `ps -T` also groups
threads beneath their process and includes unowned kernel threads; per-CPU idle
and timer threads are labelled `idle/CPU` and `ktimer/CPU`. Native workers show
service-assigned names such as `journal-admit`, `terminal-recovery`, and
`console-input-admit`; the userspace thread cleanup worker is `thread-reaper`.
Applications assign these through `std::thread::Builder::name` or rename their
current thread with `hyper_os::thread::set_current_name`. Names are diagnostic
labels; use KOIDs to distinguish threads with the same role.
`ps --name vm-runtime` filters by a name substring.
`ps -p 0x100000001,0x100000002` selects multiple processes; IDs accept
decimal or hexadecimal and print as full-width hexadecimal, matching `handle`.
Name and ID filters intersect. `--no-headers` emits rows for pipelines. The IDs are
kernel-object identities, not Unix PIDs. Use `handle` for a thread's scheduler
TID and capability details, or `top` for CPU activity over time.
