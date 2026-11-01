<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Development workflow

Create the workspace with [repo](getting-started.md). Each repository owns its
source, tests, dependency lockfiles and optional configuration. The manifest
owns checkout locations. The `build/` directory is not a Git repository:
`build/system/`, `build/app/`, `build/svc/`, `build/pi5-images/` and
`build/tools/fit-pack/` are
independent peers. Application and service implementations stay under `app/`
and `svc/`.

Use an absolute `OUT` when invoking several repositories:

```sh
make prepare OUT="$PWD/out"
make -C app/top OUT="$PWD/out"
make test MODULE=top OUT="$PWD/out"
make menuconfig OUT="$PWD/out"
```

The [build manual](https://hyper-proj.github.io/build/docs/module-builds.html)
describes registration, output ownership and module commands. See
[editor checks](https://hyper-proj.github.io/build/docs/editor.html) for the
generated Rust Analyzer workspace and
[incremental builds](https://hyper-proj.github.io/build/docs/incremental-builds.html)
for cache and disk-image behavior.

Run the affected repository's tests first. Runtime changes also need relevant
[system cases](https://hyper-proj.github.io/system-tests/index.html), using the
[test framework](https://hyper-proj.github.io/test-framework/index.html).
System CI is requested manually; related pull requests use one topic and are
submitted together through the [CI controls](https://hyper-proj.github.io/ci/docs/topic-controls.html).

Keep guides with their implementation owner. Shared
[documentation tools](https://hyper-proj.github.io/docs/index.html) validate
links and publish each repository after a merge to main. HypeR provides the
system introduction and generated module catalog rather than another manually
maintained source-file index.
