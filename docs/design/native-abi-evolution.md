<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Preparing the Native ABI for v1

This is the extension policy and pre-freeze audit of the current Native ABI.
It does **not** publish v1: `ABI_REVISION` remains zero until the explicit
release. The [generated reference](../../sdk/abi/docs/native.md) owns individual operation layouts,
rights, sizes, statuses and failure results.

## Compatibility boundary

The machine ABI consists of receiver kinds, object-local operation numbers,
register meanings, wire records, flags, statuses, capability effects and
completion/lifetime semantics. Revision denotes a compatibility generation,
not a feature counter. After v1, adding a compatible operation, flag, scalar key
or optional record tail keeps the same revision. Changing an existing meaning,
minimum prefix, authority requirement or calling convention requires a new major
contract. Unknown or unsupported operations/features must be probed, rather than
inferred from a newer build date. An old kernel may reject a newly requested flag;
it must not silently ignore a request whose semantics it does not implement.

The generic C `hyper_native_invoke6` transport retains six business argument
words, with receiver and operation passed separately. The generator now requires
every operation to leave at least one word unused. Callers zero unused words;
the kernel rejects nonzero reserved words. A future operation extension can use
a reserved word with a zero default without changing existing binaries. Busy
signatures move configuration into a sized request instead of consuming the last
register. Using the final reserved word later is wire-compatible when zero
retains its original meaning, but requires deliberately relaxing this generator
headroom check. Typed capability inputs stay visible as capability inputs.

The C/Rust SDK also exposes convenience functions. `create_file`, contiguous
VMO creation and no-follow directory opening may remain useful SDK helpers,
but each uses the canonical operation below. Adding a richer helper later does
not require changing existing helper symbols or introducing another machine
operation. In particular, a shared C library must forward the **caller's** record
size; deriving it from the library's `sizeof` would overwrite an older caller's
storage or read beyond its request after a library update. Rust wrappers that
invoke the transport directly use sizes from their own SDK build. Their Rust
dependency closure must remain coherent with that build; this machine/C-transport
contract does not promise a stable Rust compiler or Rust dylib ABI.

## Record rules

- Single-record requests carry a pointer and byte size. The supported minimum
  prefix is permanent; missing supported tail fields default to zero. Unknown
  input tail bytes must be zero. Bounds, reserved fields and unknown options are
  validated before committing side effects. Nested buffers retain separately
  documented bounds and lifetimes.
- Output queries carry the caller's byte capacity and report the current
  supported size. They write only the supported prefix that fits. Callers should
  initialize their own records to zero and check the returned supported size
  before using optional fields. There is no promise to initialize an ordinary
  query's unsupported tail.
- Directory and inspector pages carry a record stride separately from their
  element count. Strides are eight-byte aligned, at least the record's permanent
  minimum prefix and at most 4096 bytes. The kernel clips each known record to
  that stride and zeros unknown tail bytes. It never overwrites the next record.
  These remain bounded-page APIs: the published page capacity is required,
  and the result count plus opaque cursor describe actual progress.
- The shared maximum request size is 4096 bytes. No allocation proportional to
  an arbitrary user-supplied stride is needed to copy an output page.
- Existing fixed-format input arrays, such as capability dispositions and wait
  items, keep their element layout. A future incompatible element format needs
  an explicit new format/operation; appending fields to an implicit-stride array
  is not a compatible extension.
- Published reserved output words remain zero: existing typed SDK decoders may
  validate them. Add optional output fields at the tail instead of repurposing
  those words. New strided-record fields whose presence matters need an explicit
  validity indicator with zero meaning unavailable, because page results report
  a count/cursor rather than a per-record supported size.
- Object-kind identifiers and rights in handle/object observations are open
  sets. SDK readers preserve unknown kinds and rights bits from newer kernels;
  rejecting an entire page would break old inspection tools when new object
  types or permissions appear. Explicit request construction still validates
  known rights, and the kernel rejects unsupported input bits.

Startup vectors, block descriptors and filesystem-worker messages are also
kernel/userspace wire contracts. Their published offsets, tags and implicit
strides must remain stable; the sized syscall wrapper does not make an embedded
fixed-format payload extensible. In particular, a future filesystem message
header must select a new protocol explicitly through a mount-request extension,
with matching support at both endpoints. Existing mounts continue using the
current codec; increasing `HEADER_BYTES` silently would break existing workers.

## Changes before the freeze

| Operation or family | Current shape and reason |
| --- | --- |
| `directory_open_file` | `(path, path_size, requested_rights, options, mode)`. One operation covers existing files, creation, exclusive creation and truncation. |
| File-open flags | `CREATE`, `EXCLUSIVE`, `TRUNCATE` are independent bits. `EXCLUSIVE` requires `CREATE`; `TRUNCATE` requires File WRITE. Creating a file requires parent-directory WRITE but may return a read-only File. Unknown bits and invalid modes fail before mutation. |
| `directory_open_directory` | The existing options word admits `NOFOLLOW` for the final component. Conditional removal remains separate because its expected node identity prevents a lookup/remove race. |
| `vmo_create` | `(size, options)`, with zero for ordinary backing and `CONTIGUOUS` for physically contiguous backing. Both use the same ObjectFactory authority. |
| `thread_create` | `(request, request_size)`. A 64-byte prefix contains entry, initial SP, TLS, argument, affinity pointer/count, flags and a reserved word. Zero affinity pointer/count inherits the caller's allowed CPUs. Flags and reserved word are currently zero. |
| `filesystem_mount` | `(channel, buffer, queue, request, request_size)`. The 32-byte prefix contains path pointer/size, flags and a reserved word; `READ_ONLY` is the sole flag. Capability arguments remain explicitly typed. |
| `byte_channel_trace_rpc` | `(request, request_size)`. A 64-byte prefix contains phase, request/detail/generation, parent connection/request, flags and a reserved word. Copying the request may fault/block; annotations remain optional and observational. |
| Scheduling, trace configuration, root switching, firmware query | Fixed pointer-only requests now carry their byte size. Scheduling getters use the ordinary supported-size result contract. |
| Directory and inspector enumeration | Explicit output record stride permits growing result records without changing page traversal or silently changing element boundaries. |
| C record APIs | Explicit caller-supplied byte size/stride crosses the shared-library boundary; all in-tree callers and test doubles migrate together. |

Four redundant machine operations are removed: `directory_create_file`,
`directory_open_file_with_options`, `directory_open_directory_nofollow` and
`vmo_create_contiguous`. The canonical operations retain their selectors.
The schema contains 201 operations after this consolidation. All have spare
register space; complete single-record payloads require an explicit size in the
schema. Generated admission tests enforce both rules.

## Boundaries deliberately retained

The audit groups by authority, object lifetime and completion semantics, not
just similar verbs. The following remain separate operations:

| Families | Reason |
| --- | --- |
| Handles, object identity and waits | Handle authority, object identity, waiting and signal mutation have different ownership and failure behavior. |
| Byte/capability channels, wait sets and atomic waits | Byte transport, capability transfer and each wait primitive have distinct commit/cancellation rules. A generic mode switch must not hide transfer ownership. |
| Process builders, Threads, task groups and resource domains | Construction, sealing, starting, stopping and retirement are different lifecycle transitions; resource accounting is a separate authority. |
| Directories, Files, mounts and root-switch transactions | Namespace mutation, opened-file I/O, content protection and transactional publication are distinct contracts. Root-switch begin/commit/abort retain their transaction lifetimes. |
| VMO/VMAR and executable mappings | Backing allocation, placement, shared/private mapping and executable-content protection carry different rights and provenance. |
| VM construction, VM control and vCPUs | Pending construction, installed lifecycle, execution and completion remain typed transitions. |
| Devices, interrupts, DMA, block queues and transport | Claiming hardware, pinning memory, submitting I/O and acknowledging completion have separate capability/lifetime requirements. |
| System information, clocks and inspectors | Monotonic nanoseconds differ from realtime seconds/nanoseconds; task/object/memory/CPU inspection has different visibility grants. Typed queries retain bounded records. |
| Trace and crash inspection | Capture lifecycle and postmortem ownership remain explicit. Trace RPC annotations never grant capture authority. |

No networking stack or networking ABI is added for v1. x86-64 userspace trap
entry remains outside the implemented AArch64/RISC-V machine contract; this
cleanup does not claim otherwise.

## Release and future review

Before announcing v1, qualify the intended hardware and shipped SDK against the
frozen schema, publish the supported architecture/feature matrix and change the
revision only as part of that release. The present tests validate producer/
consumer agreement; they are not evidence of already released binary support.
Future changes should test both directions of record-size compatibility,
reserved/unknown fields, failure-side effects and real installed-SDK callers.
Keep new fields optional with a zero default, and preserve all existing offsets,
minimum sizes and meanings. A request requiring semantics an older kernel cannot
supply must fail visibly rather than fall back silently.

The extension rules follow the practical guidance in Linux's
[adding system calls](https://docs.kernel.org/process/adding-syscalls.html):
strict option validation and size-delimited structures with compatible tails.
Fuchsia's [VMO creation](https://fuchsia.dev/reference/syscalls/vmo_create)
provides a useful size/options precedent, and its
[object information query](https://fuchsia.dev/reference/syscalls/object_get_info)
illustrates explicit buffer extents and result counts. HypeR retains its own
object-local operation and capability model.
