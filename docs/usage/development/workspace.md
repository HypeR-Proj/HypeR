<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Workspace development

The `manifest` repository is consumed by Android `repo` and follows component `main` branches.
`repo sync` updates source checkouts. Build snapshots record resolved commits for reproducibility;
they are generated outputs, not version pins that contributors update.

## Source and build directories

Each component owns its implementation, tests, `Kconfig`, license and `component.json`.
The latter declares source ownership, build destinations and module-test commands. New files below
an owned mount are discovered automatically; moving a file across repositories requires updating
ownership in both repositories.

`HypeR/hyper prepare` assembles `out/system` using only those source repositories. The generated
layout supplies existing Make/CMake interfaces and a coherent Cargo workspace. It restores SDK
workspace dependencies and shared-library profiles without importing any original monorepo checkout.
Never edit generated files: change their owning repository and prepare again.

Application and service Kconfig entries are sourced from their component repositories. Library
configuration follows consuming components; libraries without independent switches explain this
in their own `Kconfig`. Product and board choices belong to `HypeR/products`.

The Native build proceeds through ABI generation, the C runtime, loader and Rust runtime artifacts,
then the installed SDK and applications/services. The SDK uses the loader portion of `bootstrap`;
it does not depend on a complete stage1 image. Stage1 and the normal system remain separate
compilation contexts. Runtime type identity is coordinated in the generated Cargo build context.

## Edit and test

```sh
repo start improve-top top
# Edit top/src/...
./HypeR/hyper test top
./HypeR/hyper make test-native ARCH=aarch64
cd top
git add src
git commit -s
git push hyper HEAD
```

`hyper test NAME` runs that repository's declared tests. Dependencies are resolved from the workspace;
it does not run the complete system acceptance suite. `hyper make` accepts existing Make targets.
For low-level debugging, run Make directly inside `out/system` after preparation.

For Rust Analyzer, open the generated `out/system` directory after preparing the workspace and
building the SDK. Its `.vscode` settings retain the correct target and runtime resolution context.
Keep edits in source repositories and prepare again to refresh that context. Directly opening an
individual source checkout does not provide the installed SDK or cross-repository Cargo resolution.

## Cross-repository changes

Open one PR per affected repository and add the same `topic:name` label to each. Module tests
resolve peers from that topic; they still run only the current repository's tests. After changing
peers, rerun any module checks that used the earlier combination.

In `HypeR-Proj/ci`, manually run `System verification` with `topic:name`. A single PR may instead
use `repository#number`; `main` verifies the current default branches. The coordinator resolves all
inputs before testing, rejects changed heads/bases or topic membership at completion, and writes
`system` using the dedicated GitHub App. Test jobs do not receive its private key.

Already merged topic members are accepted only when their resulting tree matches the tested
merge tree. They do not require the remaining topic PRs to repeat the same tests.

GitHub does not provide atomic merges across repositories. A successful topic run validates the
combined change; it does not make sequential merges atomic. Keep intermediate `main` combinations
compatible when coordinating multiple merges, and rerun integration if unrelated bases change.
An administrator's PR bypass is explicit and does not imply that unrun checks passed.

## Appliance and image publication

`io-vm` owns its Linux-side tests, qualified OCI publication and corresponding sources. System CI
builds the selected I/O VM source for QEMU and RPi5, then uses those exact packages in system tests.
`pi5-images` consumes the assembled source inventory and matching appliance materials. Its manual
image workflow resolves current sources automatically; source inventories accompany released images.
Libc materials are collected from the cross compiler's installed packages during appliance builds.
Old records under `pi5-images/materials/legacy` are retained only as historical provenance.

Ordinary Make builds resolve the platform main appliance channel to an immutable OCI digest.
Qualified publication advances that channel automatically. This is distinct from source-based
system verification and image publication, which build matching appliance inputs. Set
`IO_VM_PACKAGE` for local I/O VM changes or `IO_VM_REFERENCE` for an explicit published digest;
only the explicit cached-digest path works without registry access.
