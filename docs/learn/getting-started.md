<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Getting started

Install Git, Python 3.11 or newer and the Android `repo` tool. The
[build guide](https://hyper-proj.github.io/build/docs/getting-started.html)
contains compiler, host-tool and emulator requirements.

Create a workspace directly; cloning the documentation repository is unnecessary:

```sh
mkdir workspace && cd workspace
repo init -u https://github.com/HypeR-Proj/manifest -b main
repo sync
make prepare OUT="$PWD/out"
make menuconfig
make
make run
```

`make` also performs preparation automatically. `OUT` defaults to `out`; it holds
configurations, generated workspace files and build artifacts. Source repositories
remain independently editable. To change the output location, pass the same absolute
`OUT` to top-level and module commands:

```sh
make -C app/top OUT="$PWD/out"
make test MODULE=top OUT="$PWD/out"
```

Update source checkouts with `repo sync`, then build again. To select a working
manifest branch, run `repo init -b BRANCH` followed by `repo sync`.

Use the [module documentation catalog](https://hyper-proj.github.io/manifest/modules.html)
for module APIs and usage. See [system-tests](https://hyper-proj.github.io/system-tests/)
for local acceptance commands and [CI](https://hyper-proj.github.io/ci/) for explicit
cross-repository verification. Pi 5 setup is documented by
[the kernel](https://hyper-proj.github.io/kernel/docs/usage/rpi5.html).
