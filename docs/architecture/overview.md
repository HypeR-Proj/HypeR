<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# System architecture

## Design priorities

- **Rust at the kernel boundary.** The kernel is `no_std` and `no_main`.
  Assembly is limited to architectural entry, exception and context
  transitions, and instructions Rust cannot express. Panic-like convenience
  paths such as `unwrap` and `expect` are rejected by project lints.
- **Architecture is a first-class design constraint.** AArch64 remains the
  semantic priority, while RISC-V and x86-64 expose accidental coupling early.
  Common interfaces must preserve architecture-specific correctness rather
  than reducing every machine to the smallest shared abstraction.
- **Explicit ownership and lifecycle.** Page ownership, thread state, IRQ
  registration, vCPU context, publication, and rollback are designed as local
  contracts. Unsafe code is treated as an auditable implementation boundary,
  not a substitute for missing APIs.
- **Hardware-realistic foundations.** Cache maintenance, barriers, interrupt
  state, TLB invalidation, CPU startup, exception entry, and guest world
  switches are implemented with real-hardware ordering requirements in mind,
  even when QEMU cannot expose every failure mode.
- **Inspectable failure paths.** The kernel includes structured logging,
  allocation-free symbol lookup, architecture register dumps, guarded stacks,
  and an optional crash console for post-failure inspection.
- **Reproducible acceptance contracts.** Host tests, image verification, and
  QEMU guest-boot tests are normal project interfaces. Guest artifacts are
  checksum-pinned, generated under the ignored `target/` tree, and never
  embedded in the repository.

## Architecture

HypeR keeps policy above mechanism:

```text
Native userspace VMM, applications and services
    -> schema-defined pre-release syscall and capability boundary
    -> kernel user-entry adapters and services
    -> kernel policy: task, IRQ, time, memory, crash, device
    -> kernel VM policy: lifecycle, vCPU orchestration, resource ownership
    -> reusable VM formats, guest ABI, events, and device models
    -> architecture-neutral mechanisms and HAL capabilities
    -> selected architecture, firmware, and physical drivers
    -> registers, instructions, MMIO, and assembly
```

HypeR exposes the selected architecture through enforced topical context, CPU,
exception, guest ABI, IRQ, memory, platform, time, and virtualization facades.
Architecture backends own machine context, register and page-table formats,
exception entry, world switching, and hardware virtualization. Kernel and VM
services own policy, resource publication, scheduling, virtual-device binding,
and failure decisions.

Exceptions and VM exits necessarily travel upward. Named entry adapters confine
that transition, copy architecture-private state into owned typed events,
invoke immutable registered kernel services, and encode exhaustive completion
actions only after policy returns. Cargo enforces the HAL-to-core dependency
direction, Rust privacy hides architecture internals, and CI checks that graph
and rejects direct private-backend imports.

Read [the architecture guide](../../kernel/docs/architecture/README.md) for the normative
boundary rules and migration constraints. The Native contracts are specified in the
[userspace and syscall design](../../kernel/docs/design/syscall-abi.md).
