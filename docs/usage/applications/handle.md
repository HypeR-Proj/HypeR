<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Handle inspection

## Object and capability inspection

`handle` defaults to the visible object registry. Select a process by its exact
name or full KOID to list its handles. `-p PROCESS` is equivalent to the positional
argument; `--objects` explicitly selects the default registry view. Names beginning
with a digit are parsed as IDs. An ambiguous name reports the matching KOIDs so
you can select the intended process.

```sh
handle
handle shell
handle -p 0x0000000100000012
handle io-runtime --kind physical-device
handle --object 0x0000000100000034 -v
handle --all --object 0x0000000100000034
handle shell --handle 0x0f00000001000001 -v
handle --all --right write --right map-dma
handle --kind guest-memory --kind guest-mapping
handle --all --no-headers | grep physical-device
handle --list-kinds
handle --list-rights
```

The IDs above are examples; use those reported by your running system. Handle,
process and object IDs print in full-width hexadecimal; input accepts decimal
or `0x` hexadecimal; `ps` also prints full-width hexadecimal KOIDs. A handle number is local to its
process; use the OBJECT KOID to correlate the same object across processes.
Handle values are opaque and include the owning process's mask. Inspector
output shows exactly the value that process holds; do not decode it or infer
allocation order. Neither an observed ID nor this command grants access to the
observed object. See the [handle encoding contract](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/syscall-abi.md#objects-handles-and-rights).

`--kind` accepts a catalog name or numeric kind ID; repeated kinds match any of
them. Unknown names are errors. `--right` filters **granted** handle rights and
requires a process or `--all`; repeated rights must all be present. No matching
rows is a successful query with an explicit empty-result message. A specifically
selected process, object (in the registry view), or handle that is unavailable
returns failure. `--no-headers` omits banners, column headers and empty-result
messages and type-specific details for pipelines. `-v` instead adds human-readable detail and cannot be
combined with `--no-headers`.

The object table separates handle state/count from total strong references:
`unpublished` means no handle has yet been published, `active` means live handle
references exist, and `retired` means the last handle reference was released.
A retired object can remain alive through kernel references. `-v` shows the
object's supported rights and reference counts by class (kernel service, VM
device binding, scheduler, operation, user authority, publication, diagnostic,
and retirement). Supported rights are a ceiling, **not** the rights granted to
each holder; inspect process handles for that. Handle detail also includes the
raw rights mask and flags, whose interpretation depends on the object kind.
`--list-kinds` describes all SDK object types. A newer kernel kind unknown to
this SDK is still listed as `unknown(0x...)`, preserving its numeric type ID.

The default service manifests grant the session manager and shell object inspector
`INSPECT | INSPECT_DETAILS | DUPLICATE | TRANSFER`. The shell adds an inspector
with exactly `INSPECT | INSPECT_DETAILS` only when the resolved executable path
is `/bin/handle` (including the bare command `handle`).
A copy at another path does not receive it. This is launcher policy for these
trusted command paths;
the kernel checks rights and scope, never the executable name. The task inspector
passed to `handle`, `ps`, and `top` retains basic `INSPECT` only. Neither inspector
passed to `handle` has transfer, duplicate, or derive rights.

An exact `--object KOID` registry query or `PROCESS --handle HANDLE` also prints:

- Thread: scheduler TID and role, plus user-thread lifecycle when available.
  Bootstrap TID zero is valid; a thread not yet assigned a TID says unavailable.
- VMAR: address range and liveness, then current and maximum `rwx` protections
  for mappings within that region. These protections are separate from handle
  rights. Destroyed regions retain their range but no live mappings.
- Byte/capability channel: peer KOID, open state and local queue counters.
  `handle` uses ordinary process/handle scans to list visible peer holders; the
  kernel performs no reverse owner lookup. No visible holder does not prove
  that the peer is unused: handles can be in transit or outside the scope.
- Physical device: profile, lifecycle, device identity, host IRQ domain and
  interrupt range, plus host physical resource ranges and guest aperture offsets.
  This reads captured metadata, never device registers or PCI configuration.

Other object kinds keep their basic record and report that details are not
available. Detailed reads require both rights on an **object inspector**, not
`INSPECT` on the target handle. Global KOID selection requires system scope;
process-local handle selection additionally checks that process against scope.
The existing inspector derivation calls return their original base rights and
never propagate `INSPECT_DETAILS`. Target handles remain generation-qualified,
and diagnostic pins do not revive retired handles or confer operation rights.

Launchers that supply only an object inspector can use registry and numeric
process queries within its scope. Task inspection supplies process names and
holder discovery. Basic-only inspectors can still request exact table rows with
`--no-headers`; detailed reads report access denied. The catalogs and `--help`
need no inspector capabilities.

Scans are capability-scoped and weakly consistent, not atomic system snapshots.
`--all --object KOID` finds visible **process handle holders**, not every kernel
reference. A process that exits or falls outside the object inspector's scope
while scanning is skipped with a warning on stderr; other scan errors fail the
command. Already printed rows remain valid observations of their capture time.
Transient inspection references and concurrent changes mean reference totals
and separate scans need not agree exactly. An empty holder list does not prove
that an object is unused or destroyed.

VMAR detail cursors scan at most 32 mapping slots per call, outside the address-space
lock after retaining its immutable mapping snapshot. Concurrent map changes can
skip or repeat mappings; a stale cursor fails rather than implying an atomic
snapshot. All detail callbacks run after registry and handle-table locks release.

`handle --summary` counts matching registry objects by kind. With a process or
`--all`, it reports matching handle counts and distinct object counts per kind;
duplicate handles to the same object count once in the object column. Filters
apply before aggregation, and `--no-headers` also works for summary output.
Counts describe the scan's visible observations, not an atomic system snapshot.
