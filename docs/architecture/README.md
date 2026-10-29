<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Architecture design

These pages explain who owns each responsibility and how the system fits
together. Wire layouts, operations and synchronization rules live in
[detailed design](../design/README.md).

- [System architecture](overview.md): design priorities and the policy/mechanism boundary.
- [Boot, system and data deployment](boot-layout.md): bootstrap and formal environment separation, root layout and provider placement.
- [Kernel architecture boundaries](https://github.com/HypeR-Proj/kernel/blob/main/docs/architecture/README.md): crates, HAL capabilities, publication and lifecycle ownership.
- [RISC-V profile](https://github.com/HypeR-Proj/kernel/blob/main/docs/architecture/riscv64.md) and [x86-64 profile](https://github.com/HypeR-Proj/kernel/blob/main/docs/architecture/x86_64.md): architecture-specific scope and boundaries.

[Implementation status](../project/status.md) records what has been exercised;
[deployment manuals](../usage/README.md#deployment-and-configuration) explain how
to build and run a selected configuration.
