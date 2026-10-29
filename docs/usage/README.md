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

- [Product configuration](https://hyper-proj.github.io/manifest/docs/configuration.html): top-level Kconfig, kernel settings and boot-profile validation.
- [Kernel configuration](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/configuration.md): architecture and feature selection.
- [Boot profiles and mount policy](https://hyper-proj.github.io/bootstrap/docs/boot-policy.html): profile selection, fstab and recovery.
- [Board storage](https://hyper-proj.github.io/build/docs/deployment/board-storage.html): partition layout, image creation and persistent data.
- [Raspberry Pi 5](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/rpi5.md): firmware, boot, recovery and hardware qualification.
- [Image distribution](https://hyper-proj.github.io/pi5-images/docs/distribution.html): release inputs, artifacts and ownership.

## Diagnosis and measurement

- [Memory diagnostics](https://hyper-proj.github.io/kernel/docs/usage/memory-diagnostics.html): snapshots, differences and optional heap profiling.
- [PMU sampling and flame graphs](https://hyper-proj.github.io/perf/docs/profiling.html): capture, conversion and interpretation limits.
- [Perfetto timelines](https://hyper-proj.github.io/perf/docs/tracing.html): event filters, bounded capture, export and loss interpretation.
- [Scheduler benchmarks](https://hyper-proj.github.io/ci/docs/scheduler-bench.html): periodic wakeup latency, same/cross-CPU handoff, fairness and reproducible distributions.
- [Crash console](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/crash-console.md): configured post-failure inspection.
- [Kernel stack budgets](https://github.com/HypeR-Proj/kernel/blob/main/docs/usage/stack-budgets.md): measurement and reproducing the checks.
- [Storage qualification](https://github.com/HypeR-Proj/ci/blob/main/hardware/storage/README.md) and [network exercise](https://github.com/HypeR-Proj/ci/blob/main/hardware/network/README.md): hardware test procedures.

## Development

- [Development workflow](https://hyper-proj.github.io/build/docs/development.html): editor configuration, source owners and testing.
- [Incremental builds](https://hyper-proj.github.io/build/docs/incremental-builds.html): dependencies and rebuild costs.
- [Documentation maintenance](https://hyper-proj.github.io/build/docs/documentation.html): placement, website generation and link checks.
- [CI suites](https://github.com/HypeR-Proj/ci/blob/main/ci/README.md), [kernel validation](https://github.com/HypeR-Proj/kernel/blob/main/tests/VALIDATION.md), and [stress/fuzz tests](https://github.com/HypeR-Proj/ci/blob/main/stress/README.md): acceptance procedures.
