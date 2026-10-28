// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;
#[cfg(target_os = "hyper")]
extern crate hyper_tool_args_shared as hyper_tool_args;

pub mod cli;

pub mod display;
pub mod sampling;
pub mod view;

#[cfg(test)]
#[path = "../tests/fixtures.rs"]
pub(crate) mod fixtures;
