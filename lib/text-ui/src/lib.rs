// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Application-side keys, safe display cells and scoped fullscreen ownership.
//! No line discipline or terminal-service state lives in this library.
mod keys;
mod screen;
mod session;
pub use keys::{Decoder, Key};
pub use screen::{Screen, byte_at, cells, visible};
pub use session::Session;
pub type Error = Box<dyn std::error::Error>;
