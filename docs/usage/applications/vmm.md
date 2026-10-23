<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Virtual machine management

## Named virtual machines

```text
vmm list
vmm status alpine
vmm start alpine
vmm console alpine
vmm stop alpine
vmm restart alpine
vmm create test --config /data/new-vms.json
vmm create worker --config /data/new-vms.json --start
vmm delete test
```

`vmm` without a subcommand lists definitions. Every control operation requires
an explicit name from that list. Console attachment applies to that VM alone;
Ctrl-] opens the detach menu; `d`, `q`, or another Ctrl-] detaches. Any other
key prints an invalid-command notice and resumes the console without forwarding
that key to the guest. Guest input uses nonblocking sends and a bounded
64 KiB pending buffer. If the guest stops consuming input, excess keyboard data
is discarded with a warning; the detach controls remain responsive. Detaching
discards input still buffered locally. Delete requires the VM to be stopped and removes
only its definition, leaving its image intact. Stop cancels a pending restart.
Start/restart acceptance means the lifecycle request was accepted; use status to
observe `starting`, `running`, `stopping`, `stopped`, or `failed`.

Use `vmm start alpine --wait`, `vmm stop alpine --wait` or
`vmm restart alpine --wait --timeout 60` to wait for `running`/`stopped`. The
default total deadline is 30 seconds, including command admission and replies.
A failed/unavailable VM, manager error or timeout produces a failing exit status.
Timeout does not cancel or undo the operation; inspect status before retrying.
A running state proves vCPU execution, not completion of Linux boot, DHCP or
application startup. Concurrent clients can still change the VM's state; waiting
observes manager state and does not acquire an exclusive lifecycle lock.
Ordinary command exchanges also have a 30-second response deadline; attached
console sessions remain interactive without that time limit.

Each VM owns a separate runtime process, resource domain, task group, lifecycle
tracker, and console session. The current policy allows eight definitions and
budgets up to eight business VM instances plus manager overhead in a dedicated
fleet domain. The resident I/O VM uses the separate core-services domain and
does not consume business fleet slots. Actual resource
admission may fail below that ceiling. Guest memory, vCPU count and
Linux boot arguments come exclusively from each definition's `configuration`.
The default Alpine configuration uses 256 MiB and two vCPUs on AArch64 (one on
RISC-V). AArch64 supports 1..8 vCPUs; RISC-V currently supports one. RAM must be
a power of two of at least 64 MiB, subject to host resource admission.
Set `configuration.vcpus` to `4` for a four-vCPU VM; the ITB is reusable without
repacking. The runtime places initramfs at the top of configured RAM and validates
kernel, initramfs and generated DTB ranges before allocating memory.
`allocated VM backing` measures resident physical backing, not memory used
inside Linux. Attaching an I/O-backed disk or network currently populates the
guest's entire RAM VMO. See [I/O allocation and zero-copy](../../design/virtualization/io-vm.md#allocation-and-zero-copy-scope)
for the allocation policy and [memory observations](../../design/virtualization/io-vm.md#memory-observations-and-queue-overhead)
for shared-memory accounting and queue costs.
Guest PSCI poweroff stops the instance cleanly; guest reboot creates a fresh
instance under the same definition and disconnects the old console session.


## Configuration

The startup policy in `/etc/hyper/startup.json` selects a separate VM
configuration through the `fleet` field of its VM-manager application entry:

```json
{"component": "/vm-manager", "fleet": "/data/vms.json"}
```

The VM file contains definitions, not service capabilities:

```json
{
  "format": "hyper.vm-config",
  "virtual-machines": [
    {
      "name": "alpine",
      "image": "/data/vm/alpine.itb",
      "autostart": true,
      "configuration": {
        "vcpus": 2,
        "memory-bytes": 268435456,
        "bootargs": "console=ttyAMA0 rdinit=/init loglevel=7 hyper.root=/dev/sda"
      },
      "disk": { "client": 1, "volume": "alpine" },
      "network": { "client": 1, "network": "default", "mac": "02:48:59:00:00:01" }
    }
  ]
}
```

Names are unique, at most 32 ASCII letters, digits, underscores, dots, or hyphens,
and start with a letter or digit. Images use absolute paths without parent
components or control characters. Unknown fields are errors. The manager validates
and opens every image before publishing a batch of definitions. If autostart
fails afterward, the definitions remain visible with the failed VM state.

Disk and network are optional. When both are present, they must use the same
I/O client. The complete client, volume, network and MAC binding must match
the board's bootstrap policy. The example above is the runtime format; board
JSON assigns client IDs when generating these definitions.

VM configuration is managed at deployment time. Board images provide
`/data/vms.json`; update the board's VM definitions before building the image.
`vmm create` and `vmm delete` only change the running manager and do not persist
across reboot. `vmm create NAME --config FILE` reads that named definition from
FILE; `--from SOURCE` copies another named definition under NAME. Disk/network
assignments and all runtime settings are copied together; `--start` starts the
new instance. Two definitions cannot share an I/O client, an exclusive disk
volume or a network MAC address. Native test images
use `bootstrap/stage1/init/tests/config/vms.json` (or `vms-riscv64.json`).

The `vcpus`, `memory-bytes` and `bootargs` fields are required. `bootargs` may be empty and is
limited to 2048 UTF-8 bytes without NUL. Architecture comes only from the ITB's
`arch` metadata (`arm64` or `riscv`); no architecture or platform belongs in JSON.
The manager rejects images whose architecture differs from the host before
publishing their definitions. The runtime repeats this check on every start,
before allocating guest RAM, and selects the corresponding reference platform.
Architecture-specific vCPU limits are checked against the image.
The manager snapshots definitions when it loads or creates them; editing the file
does not mutate a running VM. Reload a definition by stopping and deleting it,
then creating it from the updated file, or reboot the host.

An optional `configuration.affinity` list sets default placement for selected
vCPUs, using zero-based vCPU and host CPU indices. For example:

```json
"affinity": [
  { "vcpu": 0, "cpus": [0, 2] },
  { "vcpu": 1, "cpus": [1, 3] }
]
```

Each `cpus` list is the allowed host CPU set, not a preference order. Omit the
field, use `[]`, or leave individual vCPUs out to let the scheduler assign their
host CPUs automatically. Board examples omit this optional field by default.
Lists must be nonempty with unique CPU IDs in 0..255; each vCPU may
appear once and must be below `vcpus`. All specified masks are applied before
any vCPU starts, including for I/O VMs. A mask rejected by the host (for example,
one containing no schedulable CPU) fails startup. `vmm affinity` can still change
the current instance; restarting reapplies the saved definition's defaults.
For board deployments, edit the board JSON's VM `configuration` or `io-vm`
object; generated configuration files and ITBs need no manual edits. Ordinary
builds preserve an existing disk, including `/data/vms.json`; see
[updating existing deployments](../deployment/board-storage.md#updating-existing-deployments)
to apply a changed configuration.

### Scheduling

Optional `configuration.scheduling` selects the host scheduling parameters for
every vCPU in that VM. The runtime applies them before starting any vCPU; an
invalid or unauthorized request fails startup. Omitting the whole field keeps
the kernel's vCPU defaults: Ordinary/Budget, band 128, with Q=14 ms per P=20 ms
and a maximum continuous competing turn B=10 ms in the default kernel config.
Changing these defaults in kernel Kconfig does not require copying them into
each VM definition.

To change only arbitration, omit `policy`. For example, the board I/O VM uses:

```json
"scheduling": {"band": 64, "role": "priority"}
```

This retains the kernel-selected Budget parameters. The role is explicit
deployment policy; naming a VM `io` confers no scheduling authority. Board
definitions place this field in `io-vm.configuration` or the business VM's
`configuration`, alongside `vcpus`, `memory-bytes` and `affinity`.

For an explicit Budget configuration:

```json
"scheduling": {
  "band": 128,
  "role": "ordinary",
  "policy": {
    "kind": "budget",
    "budget_ns": 14000000,
    "period_ns": 20000000,
    "burst_ns": 10000000
  }
}
```

`budget_ns` (Q), `period_ns` (P) and `burst_ns` (B) are positive nanoseconds;
Q and B must not exceed P. All Budget threads share the period configured by
`CONFIG_SCHED_PERIOD_MS`; a different `period_ns` is rejected by the running
kernel. `CONFIG_SCHED_VCPU_BUDGET_MS` and `CONFIG_SCHED_VCPU_BURST_MS` supply the
defaults. Q is a per-vCPU entitlement under contention, not a hard utilization
cap: a vCPU may borrow idle CPU time. B bounds its uninterrupted turn when
competing with other Budget threads; a sole runnable vCPU needs no periodic
scheduling interrupt. This is neither a guaranteed real-time reservation nor
an aggregate VM CPU quota. A two-vCPU VM receives two independently accounted
budgets, subject to runnable work, affinity and available physical CPUs.
When competing budgets exceed a CPU's capacity, those Q values do not guarantee
proportional shares or full delivery each period; eligible peers rotate by turn
order with B limiting a continuous turn.

`band` ranges from 0 to 255, with smaller values preferred when the algorithm's
ordering keys tie. `role` is independent of the algorithm:

- `ordinary` uses normal policy entitlement.
- `priority` uses the CPU's bounded shared priority allowance before falling
  back to normal arbitration. More vCPUs do not enlarge that shared allowance.
- `protected` participates in the CPU's shared execution floor; it is not a
  minimum share guaranteed to each vCPU.

Alternate policies are `{"kind":"fair","weight":16,"quantum_ns":5000000}`
and `{"kind":"fifo","priority":128}`. Fair weight is 1..1024: larger weights
receive proportionally more execution under stride within the same arbitration
tier. Optional `backend` accepts `default`, `stride`, or `round-robin`; omission
selects the kernel's compiled Fair backend. Changing the Fair backend currently
requires a kernel Kconfig rebuild: mixing stride and round-robin threads is not
supported, and explicitly requesting the other backend is rejected. Round-robin
does not provide weighted stride shares. FIFO priority ranges from 0 (highest)
to 255 and can starve other work, so it is intended only for trusted bounded tasks.

Configuration does not create capabilities. VM creation authority must delegate
`SCHED_CLASS`, `SCHED_PRIORITY` and/or `SCHED_BUDGET` for the requested changes;
ordinary VM `WRITE` is insufficient. `SCHED_LOWER` permits demotion, not restoration
of an earlier higher priority. The live kernel validates rights, enabled policy
and parameter limits together. Editing a definition does not alter a running
VM; recreate it from the updated definition as described above. `vmm affinity`
changes CPU placement only, not this scheduling profile.

I/O binding transfers only VM lifecycle and device-control rights to the I/O
broker. The runtime retains its already-opened vCPU handles and their scheduling
rights. The returned VM handle has no scheduling elevation rights; reopening a
vCPU through it cannot recover those rights.

See [scheduling policy and tickless execution](../../../kernel/docs/design/scheduling.md)
for arbitration, tickless deadlines, balancing and platform limitations, and
[product configuration](../deployment/configuration.md) for kernel menu settings.

Migration: rebuild old ITBs with the current packer and add `configuration` to
existing JSON definitions. The v2 ITB contract rejects v1 bundles and embedded
runtime policy; it never falls back to values from an image.

Init waits up to 60 seconds for the manager to accept the complete fleet
configuration, including an empty fleet or definitions with no autostart VMs.
The default manifest makes vm-manager noncritical: its startup failure, malformed
provisioning reply, or later exit cannot stop Native storage and core services.
A broken provisioning exchange stops that manager, while a rejected or unavailable
configuration leaves it running for inspection with VM admission disabled.
An omitted fleet configuration receives the same explicit unavailable response.
Each guest lifecycle belongs to the manager; an individual runtime failure marks
that VM failed without stopping init or unrelated guests.

I/O startup needs no business VM manager. Init creates the business resource
domain, task group and provisioning channel only when launching the manager.
Session's VMM client connection is separate from those resources. If no manager
is configured or its launch fails, init closes the unused server endpoint so
commands report unavailability promptly. The I/O broker remains a dependency of
business guest I/O; losing its manager peer stops new client admission without
stopping Native storage. A deployment can explicitly mark vm-manager critical
if it requires system-wide shutdown on that service's failure.


## Validation

`make test-apps ARCH=aarch64` checks file tools, error statuses, batch monitoring,
two simultaneous VMs, name isolation, and create/delete/restart through
the real shell. It runs as part of Native CI, alongside the existing startup and
repeated runtime-crash cleanup tests.
`make test-fleet-config` checks empty and idle fleets, atomic configuration
rejection, and an autostart failure followed by a successful independent guest.


## Console and storage visibility

Stage-1 init owns raw recovery before handoff. Formal init owns errors after
handoff and launches session-manager for interactive service. Session-manager
restarts a fresh terminal/shell session after exit or failure, with bounded
retries and a delay. It revokes the old raw binding before stopping the terminal
and session groups, waits for retirement, discards stale input, then attaches a fresh epoch.
A missing formal shell never selects bootstrap recovery. The physical Console
transport and independent background log channels survive session restarts.

On board images, `vmm list` and `vmm status NAME` include the infrastructure I/O VM
as read-only, using `io-vm.name` from the board JSON (default `io`). The snapshot reports its lifecycle, vCPU count, guest memory and current pCPU
assignment of its boot vCPU;
an unavailable management endpoint is reported as `unavailable`, not `stopped`.
The manager holds a broker observation channel for this entry, not its VM handle
or runtime control channel. `start`, `stop`, `restart`, `delete`, `affinity` and
`console` require management authority and are rejected for observation-only
entries. Authorization follows the capability held for the resolved entry;
names are not reserved by service role. The owner reports the VM name and image path from its
loaded board configuration. After I/O readiness, the manager discovers that name over
the broker, and checks for conflicts when admitting definitions and before each
instance start. A failed observation blocks admission/start instead of treating
the observed namespace as empty. Without an I/O broker, `io` is an ordinary
managed VM name. The infrastructure VM's lifecycle remains under `io-runtime`
ownership.

For a running managed VM, `vmm affinity alpine 0 1,3` sets the allowed host
CPUs for guest vCPU 0. If its current CPU remains allowed, placement is unchanged;
otherwise the scheduler chooses an eligible CPU and performs the safe handoff.
CPU lists accept comma-separated IDs and inclusive ranges, such as `0,2-3`.
This changes affinity, not guest CPU topology. The kernel's automatic Fair/Budget
balancer may subsequently move a Ready vCPU within the allowed set; select one
CPU to prevent that movement. Affinity persists across guest CPU off/on cycles.

Acceptance can precede completion of a running vCPU's handoff. `vmm status alpine`
reports each vCPU's currently assigned physical CPU (`pCPU`). Assignment does not mean
the vCPU is executing at that instant. A control timeout reports an unknown
outcome rather than claiming the affinity update was rejected.

The configuration volume at `/data` uses FAT: long filenames preserve their
spelling, while lookup compares Unicode uppercase forms and also accepts ASCII
case-insensitive short-name aliases. Names differing only in letter case are
not distinct files. This differs from ramfs, whose names are case-sensitive.
Do not assume FAT reproduces every Linux VFAT Unicode/codepage corner case.
This change does not alter either filesystem's case semantics.

QEMU loads the hypervisor image directly from the host, so the configuration
volume contains guest artifacts and configuration, not `hyper.img`. Pi 5 still
needs its firmware boot files. Existing disks are intentionally preserved by
`make` and `make run`; use `make rebuild` to reset the disk and refresh its
packaged content. Normal `make` refreshes host-side kernel and ramdisk artifacts and replaces
a changed EROFS system partition after validation, preserving bytes outside it.
See [updating existing deployments](../deployment/board-storage.md#updating-existing-deployments)
for writable system volumes, layout changes and configuration updates.

`vmm list` and `vmm status NAME` distinguish RAM capacity from **allocated VM
backing**. Allocation is a live kernel snapshot of resident primary backing:
RAM, uploaded image pages not yet accessed by the guest, and explicitly admitted
shared pools. The I/O VM currently has 64 MiB of boot RAM, a 1 MiB management pool, and a
separate 1 MiB Native storage initiator pool for each Native volume. The default
split layout has two data pools, for `hyper-data` and `hyper-system`, so its
allocated backing is 67 MiB;
each additional Native volume adds a separate shared pool. Dynamically
attached alias windows are not primary backing. The
metric excludes
the runtime process, page tables, and I/O VM mappings of other guests' memory;
it does not describe Linux's used/free memory. Repeated mappings of the same
backing range within a VM count once, while a page shared by different VMs is
attributed to each VM, so per-VM numbers must not be summed as system usage.

Inspection uses the runtime control connection and grants no additional VM
control rights. One command has a bounded observation budget shared by all
listed VMs. Starting, stopping, disconnected or busy runtimes may have unavailable
metrics; unavailable is never displayed as zero. Residency is sampled in short
locked chunks and can grow during a query, rather than freezing guest execution.

## I/O VM guest-agent metrics

For the I/O VM, guest-agent statistics show Linux CPU busy/I/O wait and memory
used, available, free and cache accounting separately from allocated VM backing.
Memory used means `MemTotal - MemAvailable`; CPU is normalized across all guest
vCPUs. Samples refresh every five seconds. An unavailable, unsupported or stale
agent is shown explicitly; the first CPU observation is marked `sampling`.
