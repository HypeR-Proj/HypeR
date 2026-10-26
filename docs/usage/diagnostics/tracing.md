<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Kernel timelines with Perfetto

`perf trace` records static kernel events into bounded CPU-local rings and exports
Chrome Trace Event JSON, which [Perfetto imports directly](https://perfetto.dev/docs/getting-started/other-formats).
Open the exported file at <https://ui.perfetto.dev>. PMU samples/flame graphs remain
available through the independent [PMU commands](profiling.md).

## Build and capture

Enable `kernel-trace` in the kernel feature menu and deploy `perf` in the system
application menu. The feature is off by default. It requires neither
`pmu-profiling` nor test features. For a QEMU development fixture:

```sh
make test-trace ARCH=aarch64
```

On HypeR, from the authorized formal shell:

```sh
perf trace record --duration-ms 5000 --categories scheduler,syscall,ipc,io \
  --cpus all --buffer-records 4096 --poll-ms 10 \
  --max-events 262144 --output /tmp/trace.json

perf trace record --duration-ms 10000 --categories scheduler,io \
  --output /tmp/storage.json -- /bin/storage-qual read /data/storage-qual.bin 101
```

The workload must be deployed separately. Recording stops when the command exits,
the recording deadline arrives, or the collector limit is reached. A running
command continues after the recording deadline; `perf` waits for it and propagates
its exit status. Children receive ordinary process capabilities, not the shell's
privileged-tool delegation; profile `vmm` with separate `start` and `dump` calls.
The deadline bounds tracing, not workload execution. Any
collector error closes recording and cleans up its own child.

For a retained-tail capture without a resident collector:

```sh
perf trace start --duration-ms 10000 --categories scheduler,vm --cpus 0-3
vmm list
perf trace status
perf trace dump --output /data/trace.json
```

`dump` stops recording before reading. `stop` is also available separately.
Both accept `--generation N` to reject an unexpected replacement capture.
Starting again discards the previous capture. Reading is non-destructive, so
another dump is possible until the next start. The kernel stops at the deadline
even if the collecting process exits unexpectedly.

`/tmp` is volatile. For bulk export, copy the completed file to a writable
filesystem and retrieve it through its backend. The QEMU harness below unmounts
its data volume and extracts the file from the stopped disk image. `-o -` emits
JSON on stdout, with diagnostics on stderr; it cannot be combined with a workload
command because child output would corrupt that stream.

## Capture controls

| Option | Default | Meaning |
| --- | --- | --- |
| `--duration-ms` | 10000 | Kernel-enforced duration, 1–300000 ms |
| `--categories` | `scheduler,vm,ipc,io` | Comma-separated categories below, or `all` |
| `--cpus` | `all` | Online indices/ranges, e.g. `0,2-3`; offline CPUs fail |
| `--buffer-records` | 4096 | Retained records per CPU, 64–4096 |
| `--poll-ms` | 10 | `record` polling interval, 1–1000 ms; snapshot mode polls child/deadline state only |
| `--snapshot` | off | `record` reads only after stopping; suitable for short workloads that fit the per-CPU rings |
| `--max-events` | 262144 | Userspace event bound, 1–4000000 |
| `--output`, `-o` | `/tmp/trace.json` | File or `-` for stdout |
| `--format` | `perfetto` | Perfetto JSON or raw `jsonl` |
| `--no-names` | off | Skip best-effort live Thread name lookup |
| `--fail-on-loss` | off | Return failure if overwrite, incomplete stream or collector limit occurred; retain output |
| `--overwrite` | off | Explicitly allow replacing an existing file |

Options belong to `start`, `record` or `dump` as appropriate; use
`perf trace COMMAND --help` for the exact syntax. `status` shows the generation,
selected CPUs, kernel identity, deadline and buffer bounds. Captures are global,
not a per-process private resource. Start/stop require CpuInspector
SET_ATTRIBUTES; reads require INSPECT_DETAILS. Copying `perf` outside the trusted
`/bin/perf` launch path does not grant authority.

## Events and interpretation

| Category | Recorded observations |
| --- | --- |
| `scheduler` | Committed switches, wait registration/completion and outcomes, wakeups and migration publication |
| `syscall` | Ordinary Native dispatch entry/return, operation and object kind, signed status |
| `ipc` | Byte/capability commit identities, delivery lengths, signal wait/notification, explicit RPC annotations and syscall-level outcomes |
| `io` | Native block submissions, completion/abort, per-device trace identity, tag, bytes and direction |
| `memory` | Native page-fault handling entry/return and resolution |
| `irq` | IRQ service entry/return with interrupt ID |
| `vm` | vCPU run entry/return to its kernel runner with exit reason |

Events are scalar observations; they contain no kernel addresses, user payloads
or raw handle values. Successful receives report delivered bytes; failed receives report zero.
Opposite channel endpoints have different KOIDs. Core `ipc_enqueue`/`ipc_dequeue`
and `cap_enqueue`/`cap_dequeue` use a shared, boot-local connection identity and
separate destination queues, plus the exact committed message/registration ID.
They also cover kernel-originated ByteChannel messages, including remote VFS
requests. These trace IDs are observations, never handles or lookup authority.
Immediate fast-path syscalls, trace-control methods and RPC annotation methods
are excluded from syscall tracing to avoid collection feedback. Other collector activity is
visible and can perturb the workload.
`syscall_exit` marks completion of ordinary dispatch: `returned=1` carries the
ABI status, including a successful yield; `returned=0` carries the requested
thread/process exit code. Subsequent yield scheduling or process teardown is
outside that dispatch interval.

CPU lanes show observed switch-to-switch execution intervals. Thread lanes show
elapsed syscall/fault/vCPU time and runnable latency. Elapsed time can include
sleep and descheduling; it is not CPU usage. vCPU boundaries are returns to the
runner, not every hardware exit handled by the HAL fast path. IRQ spans cover
ordinary routed service through acknowledgement, excluding scheduling postludes
and architecture-private RPC doorbells. Idle Threads
are named like other Threads. Thread states and scheduling classes are numeric
kernel enum values, interpreted against the included build identity.

Block spans and observed-outstanding counters describe Native block transport,
not Linux I/O VM or physical controller internals. Requests already in flight at
capture start are not included in the initial counter. Raw events, all scalar
arguments and source locations remain available by selecting an event. IDs are
hex strings to preserve 64-bit values in JavaScript; viewer lane numbers are
export-local identifiers. JSONL retains exact monotonic nanoseconds; Perfetto JSON
uses microseconds relative to capture start, retaining fractional precision.

## IPC and critical chains

Select **all CPUs**, and record at least `ipc,scheduler`. Expand thread lanes in
Perfetto and select a **Message delivery** or **RPC dispatch** slice to follow its
flow arrow. The destination includes `latency_ns`; inspect raw events for queue,
message, connection and request IDs. Equal-timestamp message boundaries match by
queue and message identity, even if CPU sorting visits the receive first; their
latency is zero. **Wake to execution** connects a committed
wait-queue wakeup to the next observed dispatch. `Blocked (notified/timeout/cancelled)`
measures wait-registration completion; **Runnable** shows subsequent scheduling
delay. These are elapsed observations, not a computed whole-system critical path.
A signal wake identifies the winning source; it does not assert which of several
queued messages caused that level signal. `waker` names the executing thread
context, which may have been interrupted by an IRQ; it is not proof that its
application initiated the notification. Persistent WaitSet notifications retain
scheduler wait timing but do not identify the original signal source.

`hyper_os::channel::RpcTracer` provides optional client/server begin/end annotations.
Construct one tracer per READ|WRITE endpoint. Both sides use the same nonzero
protocol request ID, unique among concurrent calls on their connection. The
exporter suppresses RPC spans and flows for IDs with overlapping annotations
throughout the capture, including edges preceding discovery of the overlap.
Raw annotations remain visible. Pass an
explicit `RpcContext` to a nested call to create **RPC parent** flows; no ambient
thread state or message-payload parsing infers parentage. **RPC round trip** and
**RPC handler** include completion detail, including failures. The common formal
filesystem worker and kernel remote VFS client use this interface. The bootstrap
filesystem implementation stays independent; its core messages are still traced.
No I/O VM Linux-internal spans or end-to-end physical I/O attribution are implied.

Begins/ends outside a capture, unreceived messages, aborted message preparations and
aborted receive claims never manufacture complete delivery. A stopped/replaced
capture invalidates saved RPC contexts. Duplicate active request IDs suppress
further derived RPC intervals for that ID. User annotations describe their own
protocol; they grant no ability to enable or read global tracing. No allocation,
registry lookup, payload inspection or global per-message ID counter is added to
the producer. Compiling out `kernel-trace` removes its object metadata, actor
mirror and recording paths. Formal filesystem workers then probe support once
per connection, with no per-request annotation invocations. With tracing compiled
in, explicit annotations cost an invocation per boundary even while capture is off.

The bounded development fixture exercises duplex, nested RPC and actual waits:

```sh
perf trace record --snapshot --duration-ms 30000 --categories ipc,scheduler --no-names \
  --poll-ms 1 --fail-on-loss -o /tmp/ipc.json -- /bin/lifecycle-stress trace
```

Host tests can export an import fixture, and an installed upstream Perfetto
`trace_processor` can validate its actual slice/flow tables:

```sh
HYPER_TRACE_FIXTURE=/tmp/ipc-fixture.json make app-test APP_TEST_PACKAGE=hyper-perf
python3 -B tests/profiling/verify-perfetto.py /tmp/ipc-fixture.json \
  --processor /path/to/trace_processor
```

The same import check accepts `target/app/aarch64/trace-4.ipc.json` produced by
`make test-trace`; it checks every exported arrow and rejects importer errors.
Both QEMU trace targets boot a disposable disk-backed system with a minimal I/O
VM, a system EROFS volume and a 64 MiB FAT data volume; no Alpine disk is needed.
`test-trace` covers lifecycle, loss, explicit nested RPC and a remote FAT mutation.
Its complete causal captures use one `record --snapshot` invocation: no live ring
reader contends with producers, and no start/dump command roundtrip fills the rings
with terminal traffic. Buffer capacity and loss checks still apply; snapshot mode
retains only what fits in the compiled per-CPU rings.
`make test-trace-remote ARCH=aarch64` runs only the remote filesystem scenario.

Trace bodies are never printed through the console. Capture/export writes `/tmp`
first; after recording stops, the harness copies the files through the filesystem
backend to `/data/trace-artifact-*`, then unmounts `/data` to fence data and metadata.
After the owned QEMU exits, the host validates both GPT copies and uses `mcopy`
with the selected FAT partition offset to extract the original bytes. No host
mount or privileged loop device is needed. The serial connection carries bounded
command results and capture summaries only.

The complete raw files are retained under
`target/app/aarch64/trace-<cpus>.artifacts/`; the main timeline and IPC timeline
also keep their existing `trace-<cpus>.json` and `trace-<cpus>.ipc.json` paths.
CI uploads the raw files even when host-side content validation fails. A failed
guest command retains its serial command report and the disposable disk under
`target/board-tests/trace.*/` for local diagnosis.

## Bounds and loss

Compiled-in storage is 4096 × 56 bytes per configured CPU (224 KiB plus ring
headers), even with tracing disabled at runtime. Reducing `--buffer-records`
changes the overwrite window, not this reservation. Without `kernel-trace`,
record storage and instrumentation compile out. Runtime-disabled points only
check an atomic category flag; enabled points mask local IRQs briefly and use
try-only ring access, without allocation or cross-CPU producer locking.
Feature-enabled kernels also publish one CPU-local actor scalar per context switch
and retain trace-only object/wait identities while recording is stopped.

The collector stores approximately 64 bytes per event plus interned names/sites.
It stops before exporting the file, avoiding export I/O inside the measured
interval. Large JSON output can be substantially larger than that compact
collection. Duration, ring capacity and event count are independent bounds.

Overwrites are counted per CPU. Contention/recursion/clock failure is a sticky
`incomplete` flag, not an exact dropped-event count. Loss is in capture metadata
and `TRACE LOSS`/`TRACE GAP` markers. Missing events invalidate pending spans;
any capture loss suppresses inferred causal flows and cross-CPU durations, since a missing prefix
or tail on one CPU can hide intervening operations of a migrated thread.
CPU spans on an incomplete CPU are suppressed. Selecting only part of the online CPU set also
suppresses causal flows and thread/I/O durations: a migrated thread may have unmatched boundaries
on an unselected CPU. Local CPU and IRQ intervals remain available. Neither unmatched starts nor truncated tails are
invented into complete intervals. Increase capacity, reduce categories or drain
more frequently to reduce overwrite; collecting itself also has overhead.

These are HypeR tracks in Perfetto's `slice`/`args` tables. They do not fabricate
Linux ftrace records or populate Linux-specific scheduler-analysis tables. No
live Perfetto daemon or tracing service is installed on HypeR.
