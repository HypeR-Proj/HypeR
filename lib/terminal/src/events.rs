// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Coalesced terminal events returned by the service to its clients.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventFlags(pub u32);

impl EventFlags {
    pub const INPUT_LOSS: u32 = 1;
    pub const ECHO_LOSS: u32 = 2;
    pub const SETTINGS_CHANGED: u32 = 4;
    pub const OUTPUT_RESUMED: u32 = 8;
    pub const SIZE_CHANGED: u32 = 16;
    pub const FOREGROUND_CHANGED: u32 = 32;
}
