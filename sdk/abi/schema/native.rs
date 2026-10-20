// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Compiler-checked source of truth for the `HypeR` Native ABI.
//!
//! This module deliberately depends only on `core`. Host tools, build scripts,
//! and the kernel may include it directly without acquiring a parser or schema
//! dependency. The schema describes machine-visible values; it does not contain
//! kernel dispatch policy or handler implementations.
//!
//! Start with [`AbiSchema`] in `model.rs` for the vocabulary. `capabilities.rs`
//! defines authority and identity domains; `values.rs` defines scalar domains;
//! `records.rs` fixes wire layouts. `arguments.rs` supplies reusable signatures
//! used by `invocations.rs`. `semantics.rs` records cross-operation invariants.
//! Changing a signature alone does not implement or authorize a kernel method.

#![allow(dead_code)]

/// Pre-release ABI revision.
///
/// `HypeR` does not start ABI versioning until the project explicitly publishes
/// its first supported userspace ABI. Keep this value at zero during
/// pre-release development, regardless of schema changes.
pub const ABI_REVISION: u64 = 0;
/// Business registers only; receiver and object-local operation travel separately.
pub const INVOCATION_ARGUMENT_REGISTERS: usize = 6;
/// Auxiliary results only; signed status has a separate register.
pub const INVOCATION_RESULT_REGISTERS: usize = 2;

#[path = "arguments.rs"]
mod arguments;
#[path = "capabilities.rs"]
mod capabilities;
#[path = "invocations.rs"]
mod invocations;
#[path = "model.rs"]
mod model;
#[path = "records.rs"]
mod records;
#[path = "semantics.rs"]
mod semantics;
#[path = "values.rs"]
mod values;

pub use capabilities::*;
pub use invocations::*;
pub use model::*;
pub use records::*;
pub use semantics::*;
pub use values::*;

/// Complete declaration consumed by validation, bindings and reference generators.
/// Slice order is presentation order except where a signature defines register order.
pub const NATIVE_ABI: AbiSchema = AbiSchema {
    revision: ABI_REVISION,
    features: FEATURES,
    statuses: STATUSES,
    object_kinds: OBJECT_KINDS,
    rights: RIGHTS,
    signals: SIGNALS,
    constant_groups: CONSTANT_GROUPS,
    records: RECORDS,
    invocations: INVOCATIONS,
    semantic_rules: SEMANTIC_RULES,
};

const _: () = assert!(INVOCATION_ARGUMENT_REGISTERS == 6);
const _: () = assert!(INVOCATION_RESULT_REGISTERS == 2);
