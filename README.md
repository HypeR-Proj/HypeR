<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR

HypeR is a capability-oriented operating system with a Rust kernel, Native
userspace and isolated I/O services. This repository contains project documentation
and the workspace setup helper. System implementation lives in separate
[HypeR-Proj repositories](https://github.com/HypeR-Proj).

## Quick start

Install Git, Python 3.11 or newer, the Android `repo` tool, Rust, Clang/LLVM, Make
and CMake. See the [build guide](docs/learn/getting-started.md) for host dependencies.

```sh
git clone https://github.com/HypeR-Proj/HypeR.git
cd HypeR
./setup.sh
cd workspace
make native-initramfs ARCH=aarch64
```

`setup.sh` initializes and synchronizes `workspace/` using
[the manifest](https://github.com/HypeR-Proj/manifest). To use another directory or
manifest branch, run `./setup.sh --branch BRANCH DIRECTORY`. Sources track each
repository's `main`; updating the workspace is `repo sync` from its root.

The manifest owns directory layout, product configuration and shared workspace
files. [build](https://github.com/HypeR-Proj/build) supplies build tools at the
workspace root. Edit the component checkouts directly; builds use those files.
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
cd workspace
python3 project.py test kernel
repo status
```

See [documentation](docs/README.md), [architecture](docs/architecture/overview.md),
[project status](docs/project/status.md), [contribution guidelines](CONTRIBUTING.md)
and [security policy](SECURITY.md).
