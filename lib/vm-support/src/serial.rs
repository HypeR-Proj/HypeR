// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace UART models for early kernel logs and guest panic diagnostics.

pub mod ns16550;
pub mod sbsa_uart;

#[cfg(test)]
#[path = "../tests/ns16550.rs"]
mod ns16550_tests;
#[cfg(test)]
#[path = "../tests/sbsa_uart.rs"]
mod sbsa_uart_tests;
