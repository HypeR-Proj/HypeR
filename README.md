<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR

An experimental virtualization host with a Rust kernel and capability-based Native userspace.
AArch64 is the primary platform; RISC-V remains a supported development target.

This repository contains project documentation, product configurations and the workspace helper.
Source components live in their own repositories in [HypeR-Proj](https://github.com/HypeR-Proj).

## Get the workspace

Install Git, Python 3.11 or newer, and the Android `repo` tool, then run:

```sh
mkdir hyper-workspace
cd hyper-workspace
repo init -u https://github.com/HypeR-Proj/manifest.git -b main
repo sync
./HypeR/hyper prepare
./HypeR/hyper make native-initramfs ARCH=aarch64
```

The manifest follows each component's `main`; there are no component revisions to advance manually.
It is stored by `repo` under `.repo/manifests`.

Edit the component repositories. `out/system` is a generated build view and may be replaced.
The build records the actual component commits and file hashes in `out/system/source-inventory.json`.
Rust target selection, shared runtime profiles and SDK installation remain coordinated by `build`.

```sh
./HypeR/hyper test top          # top's tests with workspace dependencies
./HypeR/hyper make test-native ARCH=aarch64
./HypeR/hyper sync
```

See [workspace development](docs/usage/development/workspace.md) for repository ownership,
configuration, editors and cross-repository changes. The [usage documentation](docs/usage/README.md)
contains build prerequisites and system commands.

## Repositories

| Area | Repositories |
| --- | --- |
| Kernel and public contracts | [kernel](https://github.com/HypeR-Proj/kernel), [abi](https://github.com/HypeR-Proj/abi) |
| SDK and runtimes | [toolchain](https://github.com/HypeR-Proj/toolchain), [lib](https://github.com/HypeR-Proj/lib), [rust-lib](https://github.com/HypeR-Proj/rust-lib), [rust-std](https://github.com/HypeR-Proj/rust-std) |
| Early userspace | [bootstrap](https://github.com/HypeR-Proj/bootstrap), [dynamic-loader](https://github.com/HypeR-Proj/dynamic-loader) |
| Applications and services | [coreutils](https://github.com/HypeR-Proj/coreutils) and one repository per other application, service or library |
| Integration | [build](https://github.com/HypeR-Proj/build), [manifest](https://github.com/HypeR-Proj/manifest), [ci](https://github.com/HypeR-Proj/ci) |
| External appliance and distribution | [io-vm](https://github.com/HypeR-Proj/io-vm), [pi5-images](https://github.com/HypeR-Proj/pi5-images) |

## Verification and contributions

PRs automatically run the affected repository's `module` tests. Run the `System verification`
workflow manually in `ci` with `repository#PR`, or give related PRs one common `topic:name` label
and dispatch that topic. System checks apply to the exact commits and dependency combination tested.

Ordinary merges require code-owner approval and both `module` and `system` checks.
Administrators can bypass these requirements through a PR. Everyone, including administrators,
is prohibited from directly pushing, force-pushing or deleting the default branch.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
HypeR code is Apache-2.0; components retain their applicable upstream licenses.
