<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# VM image and boot ownership

HypeR keeps firmware boot, system userspace, and guest boot as separate trust
and ownership layers. The platform boot chain loads the HypeR kernel and its
system initramfs and describes them through the architecture boot protocol.
QEMU uses direct kernel/initrd loading; Pi 5 uses the official EEPROM firmware
and bundled BL31. This does not imply image authentication or a secure-boot
contract. Firmware does not select, relocate or modify a guest VM.

The system initramfs is an uncompressed SVR4 `newc` or `crc` CPIO archive. It
contains `/init`, system services, SDK runtime libraries, configuration, and
zero or more guest FIT images under `/vm`. Early boot validates and reserves
the complete firmware-provided archive before making it the immutable ramfs
backing for the Native root filesystem.

## Userspace ownership

The initial service graph identifies exactly one VM manager by its validated
provisioning purpose, then starts it inside an init-created fleet resource
domain and task group. Init opens the deployment's VM configuration file and
transfers it through the provisioning channel, with a ByteChannel for initial
boot supervision. The manager validates named definitions and opens their FIT
images through its delegated Directory. Each start gets private runtime control
and console-connector channels; numeric VM IDs are not ambient authority.
Neither the manager nor a business VM runtime receives physical Console authority.

The policy allows eight named business definitions and budgets their instances,
a separate resident I/O VM and control-plane headroom. Each business child domain
allows up to eight vCPUs and 65,536 guest pages (256 MiB). These are admission
ceilings, not reserved RAM or a promise that every guest fits concurrently.
See `lib/vm-policy/src/lib.rs` for the complete limits. A stopped instance can
be recreated from its retained image with fresh resource and lifecycle owners.

For each provisioned VM, the manager creates a child resource domain and task
group, derives a one-shot VM creation lease, and starts an isolated
`/system/svc/vm-runtime` process. The guest image is duplicated into that runtime.
The runtime receives only its guest image, creation lease, read-and-execute
runtime libraries, its own process resources, and a CapabilityChannel for receiving
authorized console clients.

Each runtime:

1. parses its guest FIT through bounded random-access reads and validates the
   startup configuration supplied by the manager;
2. creates and retains the guest-memory VMO;
3. streams the selected kernel and initramfs payloads into that VMO;
4. constructs guest firmware data, including the Linux DTB;
5. seals a pending VM and atomically publishes the installed VM and boot-vCPU
   handles; and
6. retains supervision handles until acknowledged VM retirement.

The kernel owns stage-2 translation, interrupt virtualization, vCPU execution,
and teardown. It retains an independent reference to the guest-memory backing
through stage-2 invalidation, so process or handle teardown cannot free pages
still reachable by hardware. Attaching the VMO acquires an exclusive hardware
write lease: it fails if a Native writable mapping or direct access is active,
and later ordinary VMO access, snapshots, or writable mappings stay closed until
acknowledged VM retirement. Read-only Native mappings may coexist. Userspace
device backends access live guest RAM through the separate, bounded
`GuestMemory` copy operations described below.

Stage-2 demand pages are initially read/write and execute-never. A final guest
instruction fault publishes that page's instruction bytes and then promotes
only its stage-2 execute permission using the selected architecture's required
translation invalidation. Faults raised while walking guest page tables remain
data-only. Sealing conservatively publishes and promotes every loader-resident
page because executable-range metadata is not yet part of the VM ABI; sparse
pages retain the demand-promotion path.

A userspace-created VM has no implicit route to the physical host Console.
`hyper_vm_support::console::Port` starts one independently scheduled device worker
inside the owning runtime. Both `vm-runtime` and `io-runtime` use this shared
implementation. The worker owns the diagnostic UART and a single-port virtio
console, and exchanges bounded ByteChannel messages with the runtime. UART
register models live in `lib/vm-support/src/serial`; the virtio model lives in
`lib/vm-support/src/virtio_console`. Neither device depends on the Linux I/O
backend. The kernel contains generic MMIO, guest-memory and interrupt mechanisms,
not a guest UART or virtio-console model.

The installed VM admits immutable MMIO ranges before first vCPU start. Each
route has a nonzero cookie and an optional private Event. A trapped access is
staged in the vCPU, but becomes inspectable only after its hardware context is
detached. One generation-qualified request can be outstanding per vCPU;
completion validates its ID and read/write kind before the runner updates the
saved registers and resumes. AArch64 and RISC-V use the same request contract.
RISC-V legacy SBI console writes are a distinct `FirmwareConsoleWrite` request
on the registered console route, not a fabricated UART register access.

An Event is a coalesced wake prompt, not the request queue. The dispatcher clears
its Event before inspecting every vCPU's durable request slot. Publication
signals after making the slot visible; completion never clears that Event.
A VM rejects reusing one Event for another route. Event-routed requests are
excluded from the old shared vCPU MMIO signal and unfiltered query, so an I/O
controller worker cannot steal a console request or spin on its readiness.
The same SDK `DeviceDoorbell` supports other userspace MMIO device models.

The device worker reuses persistent WaitSet subscriptions for both routes,
ByteChannel and VM lifetime. After replying, it drains immediately available
requests for at most 16 scans, stopping at the first empty scan or the output
flush deadline. It does not spin waiting for the guest's next access. Each
register request completes through its userspace device model. A virtio queue
notification completes before the worker copies queue payloads, so that MMIO
write does not wait for terminal output to drain.

The console worker retains at most 64 KiB of pending output plus the UART FIFO,
bounded descriptor state and ByteChannel transport. Saturation holds
unserviceable UART writes pending and stops consuming virtio TX buffers until
space becomes available; it does not silently discard guest bytes. SBSA status,
receive and interrupt registers remain accessible while TX is full. Virtio
register access, including reset and interrupt acknowledgment, also remains live.
Output is batched in messages of up to 4 KiB, with a flush deadline 2 ms after
the first buffered byte so prompts without a newline are also delivered. Transport
backpressure waits for writability instead of repeatedly retrying an expired
flush deadline. Input is bounded as well, with FIFO capacity and UART receive
timeout modeled by the worker. Absolute monotonic wait deadlines service receive
timeouts without periodic polling. Interrupt-level updates require the VM's
WRITE management capability and reject private or out-of-range interrupt sources.
The kernel retains GIC/PLIC state and wakeup mechanisms, not UART interrupt policy.

`vm-runtime` uses a persistent WaitSet for output, control, vCPU termination,
and actionable client channel states. It owns a bounded 64 KiB retention queue
for console attachment, independently of the device worker. Without an attached
client it retains a rolling history of the most recent output. With a client,
it reserves room for a complete transport message before collecting more bytes;
a full retention queue applies backpressure rather than dropping attached
output. Control, detach and VM lifecycle events remain serviceable. `io-runtime`
formats the appliance output as boot logs. Its separate console worker services
guest devices while the I/O supervisor performs storage operations.

The shell holds only a `WAIT|WRITE` manager-connector endpoint. Each `/system/bin/vmm`
invocation obtains private control and capability channels. For `vmm console`,
the manager authorizes one client, creates a ByteChannel pair, sends one end to
the runtime's connector, and transfers the other end to the client. The manager
does not relay console bytes. Closing the private control channel releases
attachment policy; Ctrl-] detaches without stopping the VM.

The device worker retains its VM and vCPU handle owners through teardown.
Closing its runtime ByteChannel wakes a blocked worker and requests VM stop;
an unavailable device service must never leave a guest indefinitely waiting on
MMIO. Stop cancels pending requests and stale completions fail. The worker
retains its guest-memory capability until it leaves. No userspace pointer or
device FIFO is retained by the kernel.

The manager validates each runtime's monotonic lifecycle records and records
its terminal outcome in that VM's lifecycle state. Loss of either control peer or
malformed protocol data triggers an idempotent forced-stop transition; it does
not terminate the fleet manager. A client stop first requests cooperative
shutdown. The state machine also defines an explicit grace-deadline escalation
transition. The manager computes a finite absolute deadline from the Native
monotonic clock, passes it directly to the same multi-object wait, and forces
the runtime Process when the deadline expires. It does not approximate time
with polling, scheduler yields, or unrelated inspection authority.

### AArch64 guest UART

The guest device tree describes an `arm,sbsa-uart` at `0x09000000`, with INTID 33
and `current-speed = 115200`. Linux uses its existing `ttyAMA0` driver and
`earlycon` discovers the device through `/chosen/stdout-path`. The physical
host UART is independent of this guest device.

The model implements the [Arm Generic UART profile](https://documentation-service.arm.com/static/5fae4f00ca04df4095c1c988)
(Appendix B) and its [Linux device-tree binding](https://www.kernel.org/doc/Documentation/devicetree/bindings/serial/arm%2Csbsa-uart.yaml).
It has separate 32-byte RX/TX FIFOs, fixed 8N1 framing, always-enabled TX/RX,
and fixed FIFO interrupt thresholds of four bytes. It exposes DR, RSR/ECR, FR,
IMSC, RIS, MIS and ICR. It does not advertise PL011 compatibility or PrimeCell
identity, programmable baud/framing, DMA, modem signals, hardware flow control
or IrDA. Reserved registers read as zero and ignore writes.

FIFO transitions latch RX/TX interrupts; ICR acknowledges individual sources
and IMSC enables their delivery. With RTIM enabled, RX timeout fires after 32
bit periods without new input while the FIFO is nonempty. Disabling RTIM clears
the timeout and stops its timer; enabling it restarts timing for pending data.
Draining the FIFO clears the timeout; reading only part of it does not acknowledge
a pending timeout. Receive overrun status and error-interrupt acknowledgment are
independent.

This is a virtual byte stream, not a wire-timing simulation. TX drains into the
bounded output queue as space becomes available; it is not throttled to 115200
baud. That fixed baud value determines the receive timeout. A full TX FIFO
reports TXFF and clearing space updates TX status and interrupts. One worker
serializes all vCPU requests for this UART.

### Buffered guest terminal

Each reference guest also has a modern virtio-mmio console (device ID 3), with
one receive queue and one transmit queue. Both split queues allow up to 128
descriptors. The device advertises `VIRTIO_F_VERSION_1`; it does not advertise
multiport control, console-size negotiation, emergency writes, indirect
descriptors, packed rings, or event-index notification suppression.

| Guest platform | MMIO window | Interrupt |
| --- | --- | --- |
| AArch64 reference | `0x0a002000`, 4 KiB | GIC INTID 50 |
| RISC-V reference | `0x10001000`, 4 KiB | PLIC source 1 |

The device tree keeps the UART as `stdout-path`. Linux `console=ttyAMA0`
(AArch64) or `console=ttyS0` (RISC-V) therefore keeps kernel and panic messages
on the diagnostic UART. The packaged guest init separately chooses its shell
terminal: `hyper.console=virtio` requests `/dev/hvc0`; `hyper.console=uart`
selects the UART explicitly. Shipped board and test configurations select virtio;
UART remains the fallback when the argument is absent.
The Native runtime reads the same argument to route keyboard input to the
selected device. Output from both devices reaches the same console attachment.
An unavailable requested virtio driver or tty fails guest userspace startup
explicitly. Adding `console=hvc0` is unnecessary and would change the kernel's
printk-console selection.

The worker validates queue size, alignment, bounds and non-overlap before
activation. It copies and validates each descriptor chain into bounded private
state, rejects unnegotiated features and invalid directions, and prevents payloads
from aliasing either queue's metadata. Partial TX chains remain outstanding until
their bytes enter the bounded output queue. Used entries precede publication of
the used index; notification suppression is checked after that publication.
Malformed queue state sets `DEVICE_NEEDS_RESET`, reports a configuration
interrupt, and stops queue access until device reset. One worker serializes
queue access, MMIO and reset.

`GuestMemory` READ and WRITE rights independently authorize live RAM copies;
neither operation requires MAP. `guest_memory_read` and `guest_memory_write`
validate grant-relative ranges and transfer at most 64 KiB per call. Backing
ownership and copy admission survive Native user faults and handle closure.
Naturally aligned 1/2/4/8-byte guest ranges use single-copy-atomic accesses,
regardless of the Native buffer's alignment. Larger copies are not atomic
snapshots and must be validated by their consumer. Reads provide acquire
ordering; writes provide release ordering and a full memory-ordering fence before
later notifications or index loads. This lets the userspace model publish
split-ring indices without borrowing guest memory as Rust slices. Memory
dedicated to the kernel NativeBlock initiator rejects these backend copies.

## Console validation

`make test-native` covers console attachment, output, and detachment.
`make test-runtime-crash` builds the explicit `test-runtime-crash` fixture and
boots a separate archive; the production runtime keeps its default features.
Two guest definitions autostart. Attaching the first guest's console makes its
runtime exit without destructors during console forwarding; the other guest
must still answer live status requests, and the Native shell must remain usable.
A second short boot reverses the definition order and repeats the isolation
check. After stopping the survivor in the first boot, five fresh VM/runtime
cycles with the default 1 GiB QEMU RAM check teardown and continued allocation
against an idle memory baseline.
Kernel host tests verify detached MMIO publication, stale or wrong-kind
completion rejection, independent vCPU requests and cancellation on close.
A Loom test exercises clear-before-scan notification ordering with two vCPUs.
Userspace UART host tests cover FIFO ordering and saturation, interrupt
acknowledgment, receive timeout, overrun and register access boundaries.
Virtio-console host tests cover feature negotiation, queue geometry, descriptor
validation, partial TX, RX backpressure, reset, memory failures, notification
suppression, and 16-bit ring-index wrap. Guest init tests check explicit tty
selection and errors when a requested virtio console is absent.
`make test-console` checks paced guest input and exact multi-message output;
`make test-io-vm` exercises the same console worker in the I/O appliance path.

The local-only [console benchmark](../../tests/performance/guest-console.py)
compares both devices in one guest and checks every payload byte:

```sh
python3 -B tests/performance/guest-console.py \
  --image kernel/target/aarch64-unknown-none/kernel/hyper.img \
  --initramfs target/app/aarch64/initramfs.cpio \
  --output target/console-benchmark
```

Build the archive with the shipped `hyper.console=virtio` configuration first.
The benchmark drains boot diagnostics, alternates transport order, and records
artifact hashes, raw output and timings. It measures command submission through
complete receipt, including input, scheduling and terminal relay overhead.
It is intentionally excluded from CI and rejects CI environments.

An exploratory macOS ARM64 run with QEMU 11.1.2, multithreaded TCG, four host
vCPUs and 1 GiB RAM measured a 32 KiB output median of 31.97 seconds through
SBSA UART versus 1.10 seconds through hvc0 across three samples per device
(about 29 times faster). A separate 256 KiB hvc0 transfer completed in 1.57
seconds with exact payload verification. These are development-environment
observations, not hardware throughput guarantees or acceptance thresholds.

Physical AArch64 qualification still needs concurrent guest output/runtime
consumption under migration, forced runtime termination, and repeated VM
restart on FEAT_VHE-capable hardware. Check payload ordering and final memory
accounting under load; QEMU does not establish weak-memory or cache behavior.

## Guest FIT contract

Guest images use the standard flattened image tree container with embedded
payload data. The current packer emits this shape:

```text
/
  images/
    kernel@1/
      data
      type = "kernel"
      arch = "arm64"
      os = "linux"
      compression = "none"
      load
      entry
    ramdisk@1/
      data
      type = "ramdisk"
      arch = "arm64"
      os = "linux"
      compression = "gzip"
  configurations/
    default = "conf@1"
    conf@1/
      compatible = "hyper,guest-image-v2"
      kernel = "kernel@1"
      ramdisk = "ramdisk@1"
```

Kernel `load` and `entry` are exactly one 64-bit big-endian value each.
The FIT `configurations` node selects payloads only; it is not VM runtime policy.
The selected record identifies the `hyper,guest-image-v2` storage contract.
VM memory size, vCPU count and boot arguments are required in the external JSON
definition's `configuration`. The reference platform is selected from ITB `arch`.
The manager rejects a foreign architecture before publishing a definition, and
the runtime checks again at every start before allocating guest memory. The parser rejects those legacy
policy properties in a FIT and rejects v1 images. Ramdisk `load` and `entry`
are absent: the runtime computes page-aligned placement at the top of configured
RAM, so changing RAM never requires repacking. Kernel addresses remain payload
metadata checked against the Linux Image header and selected platform.
References and strings are exact, NUL-terminated UTF-8
values. Selected configuration and image records are property-only schema
leaves; child nodes are rejected. Duplicate selected nodes or properties,
embedded NUL bytes, overlapping FDT blocks, invalid ranges, unsupported image
types, and architecture mismatches are rejected before VM construction.

The AArch64 reference runtime supports one to eight vCPUs, power-of-two page-aligned RAM of
at least 64 MiB, an uncompressed Linux kernel, and an optional Linux-supported
compressed initramfs. It validates the raw Linux `Image` magic, entry and
`text_offset` placement, and nonzero `image_size`; placement reserves that
complete occupied extent rather than only the bytes embedded in the FIT. Both
the 2 MiB-aligned placement base and the occupied image extent must lie in
guest RAM. The loader rejects overlap between that extent, the initramfs, and
the generated DTB, and verifies that every selected payload range lies within
the FIT source.
FIT validation also requires an aligned, zero-terminated memory reservation
map which does not overlap the structure or string blocks. Parsing has fixed
budgets for reservation records, structure tokens, and nesting depth; embedded
payload properties are skipped in constant parser work and copied only after
the complete metadata contract succeeds. These are runtime implementation
limits, not a storage-format promise for other architectures or future VMM
implementations.

Manager admission, VM runtime validation and the I/O loader use four temporary
4 KiB windows for sparse FIT metadata reads. Each pass creates its own view;
the runtime revalidates at every start. The view is not an immutable file
snapshot and is discarded before streaming payloads. Bulk payload reads retain
their two-buffer, 512 KiB pipeline rather than being split into metadata pages.

JSON `configuration.vcpus` fixes the machine's immutable processor topology before
construction. AArch64 GICv2 and GICv3 profiles accept 1..8 vCPUs; the RISC-V
reference profile still accepts one. `pending_virtual_machine_set_bootstrap`
configures boot vCPU 0. Before installation publishes the VM, all configured
vCPU endpoints and dormant scheduler Threads are allocated and bound under the
construction transaction. Rollback owns every unpublished Thread. Installation
returns the VM and boot-vCPU handles; `virtual_machine_open_vcpu(machine, id)`
opens another configured endpoint without allocating a CPU or changing topology.
Only vCPU 0 is initially started through `virtual_cpu_start`.

## Guest power requests and SMP

AArch64 guests use PSCI over HVC. `CPU_ON`, `CPU_OFF`, `AFFINITY_INFO`,
`SYSTEM_OFF`, and `SYSTEM_RESET` are implemented alongside the PSCI discovery
calls. CPU and system suspend remain unsupported. Guest power operations never
invoke the host's PSCI firmware interface.

The kernel validates guest arguments and serializes per-vCPU power transitions.
`CPU_ON` reserves an existing powered-off CPU and records its entry/context;
the caller waits for the runtime's decision while other guest CPUs can run.
Successful acceptance supplies fresh architectural state to the target.
`CPU_OFF` parks its caller after acceptance, retaining the same Thread and
endpoint for a later `CPU_ON`. The guest CPU identity remains stable across
off/on cycles; powering off is not VM retirement. `AFFINITY_INFO` reads the
kernel's authoritative power state without a userspace round trip.

Requests are associated with their source vCPU and exposed through one VM-level
wait source. vm-runtime uses a WaitSet alongside console and control events;
there is no per-vCPU userspace waiting thread or polling timer. The Native API
adds three nonblocking VM operations, each requiring WRITE authority:

- `virtual_machine_get_power_request`: snapshot one pending request, or
  `would_block`. Inspection does not consume the request.
- `virtual_machine_complete_power_request`: accept or reject the exact request
  ID. Stale or already completed requests cannot complete a newer operation.
- `virtual_machine_open_vcpu`: return a control handle for a configured CPU.

Waiting on the VM requires WAIT authority. `POWER_REQUEST` indicates pending
work; `VCPU_TERMINATED` reports terminal CPU execution so the runtime can inspect
its retained per-vCPU handles, including failures on secondary CPUs. VM stop or
runtime loss cancels pending operations and retires every configured CPU,
including CPUs that were never powered on.

For `SYSTEM_OFF` and `SYSTEM_RESET`, the calling CPU stops guest execution while
vm-runtime coordinates whole-VM stop and waits for retirement. Poweroff reports
a clean stopped instance. Reset reports `RebootRequested` to vm-manager only
after retirement; the manager waits for successful runtime exit and creates a
fresh instance and lease. Administrative stop wins over a concurrent guest
reset. Reboot disconnects an attached console, which can then attach to the new
instance by its existing VM name.

Both GIC backends provide per-vCPU private interrupts and timers, SGI/IPI routing,
and shared SPI routing. GICv3 firmware includes one redistributor frame per CPU;
GICv2 preserves SGI source identity and its eight-bit target mask. Guest memory
supports concurrent execution while retaining per-host-CPU hardware residency.
An individual vCPU remains exclusively scheduled by its owning Thread. Mapping
changes, executable-code publication, and retirement must account for every CPU
that can retain the guest translation or instructions.

`tools/fit-pack` creates deterministic payload-only development and CI images from an
external kernel and initramfs. Generated guest artifacts remain ignored by Git.

The packer interface is `hyper-fit-pack OUTPUT ARCH KERNEL LOAD ENTRY INITRAMFS`.
It validates payload metadata and Linux kernel headers. Machine dimensions,
boot arguments and initramfs placement are checked by the runtime after combining
the image with external configuration. A single ITB can serve multiple definitions.


## Integrity and licensing

FDT structural validation is not authenticity. Production composition must
authenticate the HypeR kernel, platform DTB, system initramfs, and guest images
according to deployment policy before granting VM creation authority. FIT hash
or signature nodes can be added without moving image selection into U-Boot or
the kernel.

Guest kernels and userspace retain their upstream licenses. Downloaded guest
payloads are test artifacts and are not part of HypeR's Apache-2.0 source.
