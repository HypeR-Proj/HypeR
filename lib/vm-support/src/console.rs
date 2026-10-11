// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Runtime console service: UART diagnostics and buffered virtio terminal I/O.
//! One worker owns both devices; bounded transport keeps VM supervision separate
//! from guest output backpressure. Neither device requires the Linux I/O backend.

mod memory;
mod readiness;
mod uart;
mod worker;
pub use worker::Port;

pub const MESSAGE_BYTES: usize = 4096;
const _: () = assert!(MESSAGE_BYTES <= hyper_os::channel::MAX_MESSAGE_BYTES);

/// Keyboard destination selected by the same guest boot argument as its init.
/// Kernel `console=` remains on the UART for printk and panic output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Input {
    Uart,
    Virtio,
}

impl Input {
    pub fn from_boot_arguments(arguments: &str) -> hyper_os::Result<Self> {
        let mut selected = Self::Uart;
        for argument in arguments.split_ascii_whitespace() {
            if let Some(value) = argument.strip_prefix("hyper.console=") {
                selected = match value {
                    "uart" => Self::Uart,
                    "virtio" => Self::Virtio,
                    _ => return Err(hyper_os::Error::InvalidResponse),
                };
            }
        }
        Ok(selected)
    }
}
