<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Usage

## Applications

The [application manuals](applications/README.md) cover the shell, editor, pager,
file tools, VM lifecycle, service administration and system inspection. They
describe supported behavior and limits, not assumed Linux compatibility.

## Deployment and configuration

- [Product configuration](deployment/configuration.md): top-level Kconfig, kernel settings and boot-profile validation.
- [Kernel configuration](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/configuration.md): architecture and feature selection.
- [Boot profiles and mount policy](deployment/boot-policy.md): profile selection, fstab and recovery.
- [Board storage](deployment/board-storage.md): partition layout, image creation and persistent data.
- [Raspberry Pi 5](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/rpi5.md): firmware, boot, recovery and hardware qualification.
- [Image distribution](deployment/image-distribution.md): release inputs, artifacts and ownership.

## Diagnosis and measurement

- [Memory diagnostics](diagnostics/memory-diagnostics.md): snapshots, differences and optional heap profiling.
- [PMU sampling and flame graphs](diagnostics/profiling.md): capture, conversion and interpretation limits.
- [Perfetto timelines](diagnostics/tracing.md): event filters, bounded capture, export and loss interpretation.
- [Scheduler benchmarks](diagnostics/scheduler-bench.md): periodic wakeup latency, same/cross-CPU handoff, fairness and reproducible distributions.
- [Crash console](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/crash-console.md): configured post-failure inspection.
- [Kernel stack budgets](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/stack-budgets.md): measurement and reproducing the checks.
- [Storage qualification](https://github.com/HypeR-Proj/ci/blob/main/tests/hardware/storage/README.md) and [network exercise](https://github.com/HypeR-Proj/ci/blob/main/tests/hardware/network/README.md): hardware test procedures.

## Development

- [Development workflow](development/development.md): editor configuration, source owners and testing.
- [Incremental builds](development/incremental-builds.md): dependencies and rebuild costs.
- [Documentation maintenance](development/documentation.md): placement, website generation and link checks.
- [CI suites](https://github.com/HypeR-Proj/ci/blob/main/tests/ci/README.md), [kernel validation](https://github.com/HypeR-Proj/kernel/blob/main/tests/VALIDATION.md), and [stress/fuzz tests](https://github.com/HypeR-Proj/ci/blob/main/tests/stress/README.md): acceptance procedures.
