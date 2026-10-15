// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native terminal client API and wire contract for the formal environment.
//!
//! Applications that configure a terminal or manage foreground leases use this
//! crate. Ordinary standard-I/O byte streams need only `std::io`; they do not
//! depend on it. Terminal state, queues and line processing belong exclusively
//! to terminal-service. Bootstrap byte streams remain independent as well.

#![no_std]

#[cfg(feature = "client")]
pub mod client;
mod events;
pub mod protocol;
pub mod settings;

#[cfg(feature = "client")]
pub use client::{PreparedClient, Terminal};

pub use events::EventFlags;
pub use settings::{
    ControlCharacters, InputFlags, OutputFlags, ReadPolicy, Settings, SettingsError,
};
