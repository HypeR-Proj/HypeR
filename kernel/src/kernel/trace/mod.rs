// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Static tracepoints with flag-based runtime admission. No instruction patching,
//! timer polling, scheduler queries, formatting or output from producer context.
//! Control and export are kernel-internal in this foundation patch.

#[cfg(feature = "kernel-trace")]
mod runtime;
#[cfg(feature = "kernel-trace")]
pub use hyper::trace::{read, set_filter, start, stop};
#[cfg(feature = "kernel-trace")]
pub use runtime::initialize;
