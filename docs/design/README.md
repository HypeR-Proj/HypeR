<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Detailed design

These are the contracts used to change an implementation safely. Start with
[architecture](../architecture/README.md) for ownership, or use
[application manuals](../usage/applications/README.md) for commands.

| Area | Contracts |
| --- | --- |
| Boot | [Stage-1 handoff](https://hyper-proj.github.io/lib-bootstrap-handoff/docs/handoff.html), [kernel Native init](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/native-init.md), [VM images](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/vm-bundle.md) |
| Services | [Component supervision and routes](https://hyper-proj.github.io/component-manager/docs/components.html), [terminal service](https://hyper-proj.github.io/terminal-service/docs/terminal.html) |
| Storage | [Filesystem workers and RPC](https://hyper-proj.github.io/lib-fs-service/docs/filesystem-services.html), [asynchronous block I/O](https://hyper-proj.github.io/lib-block/docs/block-io.html), [FAT semantics](https://hyper-proj.github.io/fs-fat/docs/fat.html), [kernel VFS/cache](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/vfs.md) |
| Virtualization | [I/O VM](https://hyper-proj.github.io/io-vm/docs/hyper-integration.html), [virtio serial](https://hyper-proj.github.io/kernel/docs/design/virtio-console.html), [device/DMA authority](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/io-isolation.md), [SMMUv3](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/smmuv3.md) |
| Native ABI | [Syscall architecture](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/syscall-abi.md), [object interfaces](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/syscall-families.md), [generated ABI reference](https://github.com/HypeR-Proj/abi/blob/main/docs/native.md), [physical devices](https://github.com/HypeR-Proj/abi/blob/main/docs/physical-devices.md) |
| Kernel execution | [Timers/callouts](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/timers.md), [clocks](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/time.md), [floating-point ownership](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/floating-point.md), [random numbers](https://github.com/HypeR-Proj/kernel/blob/main/docs/design/random.md) |

Source-owner references document the [SDK](https://github.com/HypeR-Proj/build/blob/main/sdk/README.md),
[Native substrate](https://github.com/HypeR-Proj/build/blob/main/stdlib/README.md), [bootstrap loader](https://github.com/HypeR-Proj/bootstrap/blob/main/userspace-loader/README.md)
and [formal dynamic loader](https://github.com/HypeR-Proj/lib-dynamic-loader/blob/main/README.md).
