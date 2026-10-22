// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Build-time kernel configuration generated from `.config`.

include!(concat!(env!("OUT_DIR"), "/kernel_config.rs"));

/// Runtime stack slots for every schedulable Thread except bootstrap (which
/// owns its boot stack), plus each CPU's IRQ and emergency stacks. This is a
/// derived capacity, not an independent limit that can starve the scheduler.
pub const KERNEL_STACK_SLOTS: usize = SCHED_MAX_THREADS as usize - 1 + 2 * MAX_CPUS as usize;
