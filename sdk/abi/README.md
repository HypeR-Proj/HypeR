<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Native ABI source

HypeR ABI is the compiler-checked source of truth for the HypeR Native capability invocation
boundary. It provides dependency-free Rust definitions for `no_std` consumers,
a C/C++ header, invocation metadata, wire-layout assertions, and an auditable
reference generated from one schema.

The Native ABI is intentionally independent of kernel implementation details
and language runtime policy. Foreign OS compatibility interfaces are not
defined by this component and are not implemented Native SDK capabilities.

HypeR is pre-release. The ABI revision remains zero until the project explicitly
publishes its first supported ABI; schema changes before then do not imply
stability.

See the [Native ABI reference](docs/native.md) for operation selectors, arguments,
record layouts, rights and failure results.

## Repository layout

| Path | Responsibility |
| --- | --- |
| `schema/native.rs` | Schema entry point, revision and register counts |
| `schema/model.rs` | Core-only schema vocabulary and contract types |
| `schema/capabilities.rs` | Status, object kinds, rights and signals |
| `schema/values.rs` | Named enum, flag and numeric-parameter groups |
| `schema/records.rs` | Wire records, field offsets and minimum prefixes |
| `schema/arguments.rs` | Reusable receiver, argument and result contracts |
| `schema/invocations.rs` | Object-local operations and capability effects |
| `schema/semantics.rs` | Cross-operation invariants |
| `src/generated.rs` | Generated dependency-free Rust values and layouts |
| `include/hyper/native.h` | Generated C and C++ interface |
| `docs/native.md` | Generated invocation and object reference |
| `generator/mod.rs` | Generation, drift checks, and repository output |
| `generator/validation/` | Scalar namespaces, record layouts, and invocation ownership contracts |
| `generator/render/` | Deterministic Rust, C, and reference renderers |
| `generator/names.rs` | Shared spelling of generated public identifiers |
| `tests/` | Rust, C, and C++ layout conformance |

## Use from Rust

The `hyper-abi` crate is `#![no_std]` and has no dependencies. The kernel uses
the in-tree package directly:

```toml
[dependencies]
hyper-abi = { path = "sdk/abi" }
```

## Generate and verify

```sh
cd sdk/abi
cargo run --features generator --bin hyper-abi -- write
cargo run --features generator --bin hyper-abi -- check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Generated files are reviewed and committed. CI rejects drift between the schema
and generated Rust, C, and reference outputs.

## Organizing scalar values

Related values belong to a named group in `schema/values.rs`. The declarations
use Rust macros with C-like enum syntax; no separate schema parser is required:

```rust
constant_groups! {
    /// Process lifecycle phases.
    enum process_phase {
        process_phase_prepared = 0,
        process_phase_created = 1,
        process_phase_running = 2,
        process_phase_stopping = 3,
        process_phase_stopped = 4,
        process_phase_retiring = 5,
        process_phase_retired = 6,
    }

    /// Permissions which may be combined for a mapping.
    flags vmar_permission {
        vmar_permission_read = 1 << 0,
        vmar_permission_write = 1 << 1,
        vmar_permission_execute = 1 << 2,
    }
}
```

Use `enum` for mutually exclusive states/selectors, `flags` for independent
bits, and `constants` for limits, sizes, addresses and sentinels. Validation
rejects duplicate enum discriminants, zero/overlapping/composite flag members,
duplicate group names and public-name collisions across groups. Repeated values
are allowed in numeric-parameter groups.

The groups describe scalar domains, not Rust enum fields on the wire. C and
Rust exports retain their existing names and integer types, including full
64-bit masks and sentinels. Consumers still validate untrusted integers before
decoding them. Group comments and individual tables are included in the
generated reference; operations and records each have their own section.

Rights in `schema/capabilities.rs` likewise have one declaration per bit:
the same declaration supplies both schema metadata and masks used by operation
contracts. Do not add a second, separately maintained mask definition.

## API documentation

The generator emits Rustdoc for Rust exports and Doxygen comments for the C
header, including named scalar groups. Both reuse the operation metadata to
describe receiver authority, handle ownership, user-memory bounds, register
order and results valid on failure. Record documentation identifies consuming
operations and distinguishes a supported minimum prefix from a call's required
buffer size. The C API also includes the schema's cross-operation semantic rules.

Add member-specific units, sentinels or lifetime constraints directly above a
scalar declaration; multiple `///` lines are supported:

```rust
/// Wait without a deadline; finite deadlines use absolute monotonic nanoseconds.
deadline_infinite = u64::MAX,
```

These comments appear in Rustdoc, Doxygen and the Markdown reference. Do not edit
generated comments separately. Semantic rules are plain prose; the renderers
escape angle brackets so path placeholders and comparisons remain visible.
The documentation site retains `EXTRACT_ALL = NO`: undocumented C implementation
details are not exposed merely to make the ABI visible.

Validate Rustdoc with `cargo doc --no-deps`. The repository documentation build
uses `scripts/docs/Doxyfile` for C API extraction.

## Ownership boundary

- The kernel implements and validates the machine contract.
- `stdlib/hyper/` provides the C runtime and architecture invocation veneers.
- `stdlib/rust/` provides raw and safe Native Rust application bindings.
- `sdk/toolchain/` assembles the compiler, linker, headers, and runtime into a
  consumable SDK.

## License

Licensed under the Apache License, Version 2.0. See
[the project license](../../LICENSE).
