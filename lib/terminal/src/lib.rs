// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native terminal policy. Bootstrap byte streams do not depend on this crate.
//!
//! The discipline is independent of IPC and clocks: the owning reactor supplies
//! monotonic timestamps and commits reads only after successfully sending their
//! response. Terminal control is separate from the generic standard-I/O stream.

#![no_std]

extern crate alloc;

#[cfg(feature = "client")]
pub mod client;
pub mod discipline;
pub mod protocol;
pub mod settings;

#[cfg(feature = "client")]
pub use client::{PreparedClient, Terminal};

pub use discipline::{Discipline, EventFlags, Limits, ReadDecision, ReadRequest};
pub use settings::{
    ControlCharacters, InputFlags, OutputFlags, ReadPolicy, Settings, SettingsError,
};
