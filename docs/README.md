<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Documentation

Choose a section by the question you want to answer. The repository and the
[documentation website](https://roolrz.github.io/HypeR/) use the same source pages.

| Section | Purpose | Start here |
| --- | --- | --- |
| [Learn](learn/README.md) | Build a first image and learn the code paths. | [Getting started](learn/getting-started.md) |
| [Architecture design](architecture/README.md) | Understand system boundaries, ownership and boot organization. | [System architecture](architecture/overview.md) |
| [Detailed design](design/README.md) | Work on contracts, protocols, state transitions and subsystem invariants. | [Bootstrap handoff](design/boot/bootstrap-handoff.md) |
| [Usage](usage/README.md) | Configure, deploy, operate and diagnose a system. | [Application manuals](usage/applications/README.md) |
| [Project](project/README.md) | Check implementation coverage, roadmap and contribution policy. | [Implementation status](project/status.md) |

Generated Rust and C API references are available from the website's **API
reference** menu. Source-directory READMEs describe their own build and code
organization; the **Source references** menu groups them by repository owner.
Kernel-owned guides remain under `kernel/docs/`, categorized alongside the system
guides in the website navigation.

For documentation placement, links and publishing checks, see
[maintaining documentation](usage/development/documentation.md#where-a-page-belongs).
