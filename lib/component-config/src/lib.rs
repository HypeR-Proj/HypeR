// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded component declarations and capability routing. This crate has no
//! kernel bindings: image composition and runtime admission use the same rules.

mod model;
mod topology;

pub use model::*;
pub use topology::{Instance, Route, RouteStep, Topology};

pub const MAX_COMPONENTS: usize = 64;
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024;
pub const MAX_CONFIGURATION_BYTES: usize = 128 * 1024;
pub const MAX_ROUTES: usize = 256;
pub const MAX_DEPTH: usize = 12;

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

pub fn absolute_path(path: &str) -> bool {
    path.starts_with('/') && path.len() <= 255 && !path.as_bytes().contains(&0)
}
