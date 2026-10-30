<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR

HypeR is a capability-oriented operating system with a Rust kernel, Native
userspace and isolated I/O services. This repository contains project documentation
only. System implementation lives in separate
[HypeR-Proj repositories](https://github.com/HypeR-Proj).

## Quick start

Install Git, Python 3.11 or newer, the Android `repo` tool, Rust, Clang/LLVM, Make
and CMake. See the [build guide](docs/learn/getting-started.md) for host dependencies.

```sh
mkdir workspace && cd workspace
repo init -u https://github.com/HypeR-Proj/manifest -b main
repo sync
make prepare OUT="$PWD/out"
make native-initramfs ARCH=aarch64
```

The [manifest](https://github.com/HypeR-Proj/manifest) owns source locations and
product definitions. Sources track each repository's `main`; update with `repo sync`.
For a working manifest branch, select it with `repo init -b BRANCH`, then sync.
There is no need to clone this documentation repository into the workspace.

[build](https://github.com/HypeR-Proj/build) supplies build tools at the workspace
root. `make` runs preparation automatically; `make prepare` is also available
explicitly. Generated Cargo workspaces, editor settings, configurations and build
artifacts live in `OUT` (default: `out`). Sources remain in their owning repositories.
Library repository names start with `lib`, including `lib-system`, `lib-rust` and
`lib-rust-std`. I/O VM packages and Pi5 images have separate publication repositories.

## Development

Each repository owns its license, CODEOWNERS, module configuration and tests.
Pull requests run module tests automatically. Related PRs share a lower-case
`topic:` label. In the PR Checks tab, use **Run system CI** and **Submit topic**
to verify and submit the complete group. The central coordinator processes button
requests every few minutes. See [topic controls](https://github.com/HypeR-Proj/ci/blob/main/docs/topic-controls.md)
for submission policy, recovery and expired-label cleanup.

```sh
make test MODULE=kernel
make -C app/top OUT="$PWD/out"
repo status
```

See [documentation](docs/README.md), [architecture](docs/architecture/overview.md),
[project status](docs/project/status.md), [contribution guidelines](CONTRIBUTING.md)
and [security policy](SECURITY.md).
