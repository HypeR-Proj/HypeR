<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Implementation status

[Project overview](../../README.md)

This is the detailed inventory of implemented foundations and their current
acceptance boundaries. Planned work is tracked in the [roadmap](roadmap.md).

| Host architecture | Status | Current acceptance contract |
| --- | --- | --- |
| AArch64 | Tier 1 | QEMU `virt` with FEAT_VHE required; LSE; SMP; Linux guest reaches `/init` and provides an interactive console |
| RISC-V 64-bit | Supported | QEMU `virt`; kernel self-tests; UP/SMP Native applications and userspace-managed Linux guests, interactive console and VM retirement |
| x86-64 | Experimental | QEMU `q35`-targeted build and image validation; no public runtime contract yet |

On Pi 5 D0, hardware bring-up has confirmed the official EEPROM boot path,
four host CPUs, interrupt-driven debug UART, Linux I/O VM userspace and FAT
directory reads through the SDIO1/dm-linear/vhost-scsi path. A two-vCPU Alpine
guest has passed three CPU 1 off/on cycles, ordinary reboot with a synced ext4
file preserved, and ordinary poweroff with the I/O VM and `/data` still
available. Those lifecycle results precede the combined SDIO1/RP1 deployment.
On 2026-10-06, the combined deployment completed 1 GiB Native FAT and guest ext4
writes/overwrites with full readback, plus bidirectional guest TCP measurements
against a Mac peer. The [reference data](../../README.md#exploratory-pi-5-io-measurements)
uses an informal environment and one run per case. Power-loss durability,
concurrent and sustained load, and physical DMA retirement under faults remain
unqualified and are tracked as
[deferred qualification](roadmap.md#deferred-hardware-qualification).
The current priorities are best-effort block/guest-network tuning without major
code or architecture rework, and capability/DMA isolation hardening around the
I/O VM. The default QEMU deployment now connects physical virtio-PCI assignments
to [host-owned SMMUv3 domains](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/smmuv3.md), with retained RAM and
shared-buffer leases, completed invalidation, IRQ-driven stream quarantine and
fail-closed controller handling. This integration does not establish DMA
containment for the MMIO compatibility or Pi 5 deployments. See the
[I/O authority and DMA ownership contract](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/io-isolation.md) for
implemented protections and remaining qualification gaps.
See the [Pi 5 guide](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/rpi5.md) and
[image release contract](../usage/deployment/image-distribution.md).

The current foundation includes:

- position-independent boot images and Linux-compatible architecture entry;
- FDT-based platform discovery on current QEMU hosts, with Linux-compatible
  architecture handoffs including x86 boot parameters;
- KASLR with RELA/RELR relocation on AArch64;
- permanent stage-1 mappings, guest stage-2 translation, boot allocation,
  buddy allocation, slab allocation, and a bounded per-CPU fast path behind
  Rust's global allocator interface;
- SMP startup with a scheduler-owned idle thread on every admitted CPU;
- intrusive indexed ready queues with FIFO, weighted-stride Fair (or a
  configured round-robin backend), and per-vCPU Budget scheduling; arbitration
  and scheduling authority are described in the [scheduler design](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/scheduling.md);
- tickless scheduler deadlines, load-aware initial placement and bounded
  event-driven balancing of Ready Fair/Budget threads within their CPU affinity;
- explicit affinity-controlled kernel-Thread migration with source-context
  completion before target publication, including blocked waiters and affinity-driven vCPU migration;
- generation-tagged, allocation-free wait arbitration across notification,
  timeout, and cancellation, with counted Completion, sleeping Mutex and
  Semaphore primitives, and migration-safe deadline waits;
- an in-tree, schema-defined pre-release [HypeR Native ABI](https://github.com/HypeR-Proj/abi/blob/main/docs/native.md)
  with checked Rust and C layouts, syscall metadata, and an auditable reference;
- a compiled capability foundation with fallible shared objects, schema-owned
  rights, 64-bit generation handles, detached unpublished slot transactions,
  deferred close, allocation-free iterative teardown, and weak global
  object/Process discovery with immutable, scope-derived task and object
  inspectors plus bounded pointer-free handle-graph snapshots;
- capability-backed Event objects with independent `WAIT` and `SIGNAL`
  authority, absolute-deadline waits, and exactly-once arbitration among
  signal, timeout, and Process cancellation;
- bounded byte-channel endpoints with FIFO messages and level signals, plus
  synchronous capability rendezvous with typed receive contracts;
- strong `ProcessImage`, `Process`, object-backed `UserThread`, and `TaskGroup`
  ownership with accounted construction, explicit publication, start/ready,
  stop/join, and acknowledged retirement;
- Native user-thread creation/start/stop, process-private atomic wait/wake and
  scheduler-backed sleep, with Rust std spawn/join, TLS cleanup and detached
  stack reclamation through the shared runtime;
- a writable kernel ramfs with rooted traversal, links, rename, metadata and
  advisory locks, optional RTC-backed UTC, a small kernel bootstrap contract,
  an AArch64/RISC-V static bootstrap loader, and an explicitly delegated formal
  executable loader/runtime linker for system programs, with eager relocation,
  W^X/RELRO enforcement, guarded stacks, and
  scheduler-owned Process publication;
- independent File `PROTECT` authority and process-local protection tokens;
  private file mappings retain stable content identity and reclaim clean pages
  for later refault, while relocations and other writes remain private. Generic
  memory-VMO snapshots still support copy-on-write or eager private mappings.
  Shared writable VMOs and guest memory retain stable physical backing; this
  is not live address-space cloning or fork;
- manifest-driven stage-1 and an independent formal component-manager and finite startup coordinator;
  formal component declarations, readiness, bounded restart, volatile journal
  capture and scoped status/control tools construct services transactionally,
  transfer core supervision once, and delegate monotonically attenuated
  capabilities; a separately authorized, atomic VFS root switch retains the
  original RAM root at `/boot`, preserves live file and mount identities, and
  precedes system-service launch. System files live on an independent disk
  filesystem, EROFS by default;
- capability-scoped Native `ps` and `handle` tools with bounded task names,
  explicit Process-to-Thread ownership, decoded object purposes and rights,
  and KOIDs that remain diagnostic correlation values rather than authority;
- an AArch64 VHE native-EL0 proof which enters through a scheduler-owned
  user Thread, dispatches the initial handle, scheduling, lifecycle, and Event
  syscalls, contains a user fault, and retires the complete Process ownership
  graph;
- safe AArch64 IRQ-tail preemption, including deactivation and resumption of
  scheduler-owned vCPU continuations;
- IRQ domains and shared handler registration, host/guest GICv2 and GICv3, PLIC, x2APIC, host
  and virtual architectural timers;
- host PSCI and SBI CPU-power backends;
- AArch64 guest SMP with 1..8 vCPUs on GICv2/GICv3, per-vCPU timer and IPI
  state, and runtime-mediated PSCI CPU on/off and VM poweroff/reset; guest
  suspend remains unsupported, and RISC-V guests currently retain one vCPU;
- a compatibility-matched platform driver framework, PL011 and NS16550 UARTs,
  and reusable virtual-device models;
- board guest FIT images loaded from `/data`, with only the I/O VM FIT in the
  bootstrap initramfs; standalone Native acceptance archives retain guest fixtures;
- rollback-safe VM construction with one generational registry publication for
  guest memory, virtual interrupts, devices, and all configured dormant vCPUs;
- a capability-scoped Native VM manager contained by a bounded fleet resource
  domain, plus isolated per-VM runtimes which parse FIT images, own guest VMOs,
  construct Linux firmware data, and supervise installed VMs from userspace;
- a multi-client VM control plane and `/bin/vmm` lifecycle client, with one
  exclusive runtime-provided console connection per VM, caller-allocated shared
  output pages, and userspace retention independent of the physical Console;
- boot-relative, severity-tagged kernel log buffering, lock-independent Thread
  name snapshots for diagnostics, kallsyms, guarded kernel/IRQ/emergency
  stacks, and an optional allocation-free crash console.

This list describes implemented foundations, not a claim of production
completeness. In particular, general-purpose VMM resource policy, device assignment, strong
guest isolation policy, cross-architecture asynchronous preemption, topology-aware load balancing, broad hardware discovery, stable
management APIs, and a general-purpose virtual I/O stack are still under
development.

## Linux I/O VM baseline

The [I/O VM integration](../design/virtualization/io-vm.md) retains a dedicated AArch64 QEMU MMIO
cross-VM storage compatibility fixture. Unmodified upstream Linux 6.18.51 and
external modules run in a trusted I/O VM; it directly drives a QEMU virtio-scsi
disk and exports it through upstream vhost-scsi/LIO iblock. A separate Linux
guest uses its ordinary virtio-scsi
block driver. The Native fixture verifies 32 direct write/flush/read rounds,
host disk bytes, and restoration of Native memory access after both VMs retire.
Both GICv2 with one host CPU and GICv3 with four host CPUs pass driver
unbind/rebind followed by repeated I/O, exercising backend endpoint reset and
a new notification epoch. Each guest in this isolated storage fixture has one vCPU; board Alpine
deployment uses two on AArch64.

The implementation includes capability-gated physical device claims, explicit
DMA translations, shared guest-memory grants, bounded asynchronous per-vCPU
configuration MMIO, control mailboxes, and direct kernel kick/call notification
bindings. Native management services do not forward individual disk requests.
AArch64 GICv2/GICv3 tests also exercise mailbox interrupts, cross-VM notifications,
peer exit, stale completion rejection and interrupted MMIO retirement.

Linux build, modules, services, rootfs assembly and source packaging live in
[HypeR-io-vm](https://github.com/HypeR-Proj/io-vm). HypeR owns Native deployment,
DT generation and digest-pinned import. Default AArch64 `make run` keeps the
Native shell and starts the resident I/O VM, whose shared queues back separate
EROFS system and FAT configuration volumes. Stage-1 switches the independent
system filesystem from its temporary `/system` mount to `/`; formal init mounts `/data`.
The manager loads `/data/vms.json`;
`vmm start alpine` boots its configured disk-backed guest. `make` prepares these
artifacts, while `make run` only launches them. The optional `RUN_PROFILE=io`
standby profile deliberately omits Native storage and business clients.

The default QEMU board instead assigns physical virtio-PCI SCSI and network
functions behind SMMUv3. The kernel installs the I/O VM's DMA domain before
enabling bus mastering, admits only its RAM and explicit live backend grants,
and retains backing until DMA invalidation completes. Separate operation rights
gate device inspection, assignment, DMA-address disclosure, register access and
interrupt acknowledgement. These mechanisms protect ungranted HypeR memory;
guests still trust the backend for data and availability. In addition to
functional PCI storage/network acceptance and the EDU DMA-denial fixture, a
[production NIC test](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/smmuv3.md#production-io-vm-mapping-revocation)
revokes a warmed Linux RX buffer mapping and verifies actual DMA write denial,
unchanged destination bytes, continued storage DMA and journal reporting across
bootstrap handoff. Other device/read paths and physical hardware retirement
remain outside that test's coverage.

`board-storage` tests persistent Native files, Alpine ext4 root, ordinary VM
disks, broker isolation and userspace device access. `test-io-vm` checks the
isolated two-VM data path; `test-io-standby` checks idle backend operation.
Pi 5 SDIO1 assignment is implemented in Native userspace. The complete RP1
PCI function is assigned through a generic PCI configuration/BAR/MSI-X
transport. Linux owns RP1 interrupts and peripheral drivers; the host retains
the BCM2712 PCIe bridge and physical MSI controller. This trusted I/O VM path
has no IOMMU DMA isolation. Basic physical guest networking has been exercised;
sustained/fault qualification, automatic recovery of quarantined devices and
fault-time DMA retirement remain unfinished.
QEMU does not qualify physical cache, interrupt or DMA behavior.

## Guest virtio-net implementation

The QEMU board selects a modern PCI virtio frontend with INTx and Linux
vhost-net/TAP; configurations omitting `virtio-transport` still default to MMIO.
The frontend is bridged to an assigned virtio-net uplink and QEMU user
networking. A VM can have
a disk, a network interface, or both. Both devices share one guest-memory grant
and mailbox, with separate notification bindings and reset epochs. All vhost
users must drain before the shared mapping can be released. Client zero owns physical-block management and has no active network frontend
or link; Native networking remains reserved for future work.

The reference GICv2/GICv3 controller exposes 256 interrupt IDs, and physical
assignment supports multiple controllers with common VM publication and
retirement. The network device has one RX/TX pair, a configured MAC and MTU
1500. Guest TX checksum and TCPv4/TCPv6 segmentation can be negotiated with an
offload-capable I/O VM. Both pinned upstream appliances advertise these optional
features; default builds require no local backend override. RX offloads and
multiqueue remain disabled. See the [network contract](../design/virtualization/io-vm.md#guest-networking) for the header
ownership and software fallback requirements.

Host configuration/state-machine tests and Linux control/retirement-failure
tests pass. Manual AArch64 QEMU TCG acceptance passes with both GICv2 and GICv3:

- A disk+network guest and a network-only guest obtain separate DHCP leases.
- Each downloads a 256 KiB + 137 byte host-served payload and verifies SHA-256.
  The TX offload test also uploads it, requiring an independently computed
  checksum receipt, and verifies negotiated feature bits after each activation.
  An additional public HTTP fetch checks DNS and outbound connectivity.
- Each network driver unbinds/rebinds and transfers again while the other VM
  remains running; both guests also pass stop/start and repeat transfers.
- The disk guest retains its proof file; the network-only guest has no SCSI
  device. All four VM stops receive `RELEASE_MEMORY: ok`.

Alpine now brings up its interfaces and runs a background DHCP client after
mounting its final root. Automatic address, route and DNS configuration, DHCP
client liveness, HTTP checksums and repeated start/stop have also passed on the
default GICv3 QEMU deployment. Separate guest boot checks confirm that a missing
NIC or DHCP server does not block the shell.

The QEMU results above are functional tests without performance measurements. The
TX offload implementation has passed both GIC variants and fallback with the
previous appliance generation. Its Linux packet fixture also passes TCPv4/TCPv6
software segmentation and checksum verification on both kernel profiles.
The 2026-10-10 manual Pi 5 survey reached about 935–936 Mbit/s guest TX after
fixing vhost work-node reuse ordering. Single-connection 60- and 120-second TX
runs and one/four-connection bidirectional cases completed without a stall.
These results used a local build of the same appliance source tree; they do
not qualify the exact published binary or establish long-duration stability.
See the [Pi 5 I/O measurements](../../README.md#exploratory-pi-5-io-measurements) for the
comparison and its limits.

The Pi 5 development image also passed a physical smoke test after PCI host window
setup was corrected. The implementation assigns the whole RP1 PCI function to
Linux, with a separate noncoherent DMA bus and exclusive ownership of the RP1
shared infrastructure. The published pin subsequently served the 2026-10-06
physical storage and TCP performance survey. Full hardware qualification,
stress behavior and DMA retirement remain separate;
see [Ethernet qualification](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/rpi5.md#ethernet-backend-qualification).

This control plane uses protocol version 3 of the independent Linux appliance.
The package lock selects the published QEMU and Pi 5 generations from the
merged upstream source. Ordinary builds import these immutable GHCR digests.
See [build and validation](../design/virtualization/io-vm.md#build-and-validation) for the QEMU deployment
and package boundary.

## Runtime qualification

Kernel self-test images contain no Linux guest loader or default VM policy.
AArch64 and RISC-V Linux integration uses Native `vmm create/start/console` and
validates guest userspace startup, console input, named VM isolation and runtime-loss
retirement. RISC-V also retains a separate Native-authority guest fixture for
stop, WFI wakeup, virtual UART/PLIC, privilege isolation and retirement.
The x86-64 build gate does not establish a userspace guest-boot contract.

Pi 5 host bring-up prerequisites and recommended firmware configuration are
documented in [Raspberry Pi 5](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/rpi5.md). GICv2 Linux guest boot
and lifecycle tests run in QEMU; physical Pi 5 validation has established the
bounded hardware results listed above. Broader stress and fault qualification
remain outstanding.
The guest SMP acceptance target covers affinity-driven vCPU migration between host CPUs,
timer progress across migration, secondary CPU off/on cycles, concurrent
work, guest reboot/poweroff and reclamation on both GIC backends, including a
four-vCPU guest on a single-host-CPU configuration. This checks integration,
not physical cache, TLB or interrupt ordering.
