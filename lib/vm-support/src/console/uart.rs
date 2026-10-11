// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Common worker adapter for the architecture-specific diagnostic UART.

use crate::serial::{ns16550::Ns16550, sbsa_uart::SbsaUart};
use hyper_os::{Error, Result, vm};

pub(super) enum Model {
    Sbsa(SbsaUart),
    Ns16550(Ns16550),
}
impl Model {
    pub(super) fn can_receive(&self) -> bool {
        match self {
            Self::Sbsa(uart) => uart.can_receive(),
            Self::Ns16550(uart) => uart.can_receive_external(),
        }
    }
    pub(super) fn receive(&mut self, byte: u8, now: u64) {
        match self {
            Self::Sbsa(uart) => {
                uart.receive(byte, now);
            }
            Self::Ns16550(uart) => {
                uart.receive_external(byte, now);
            }
        }
    }
    pub(super) fn advance(&mut self, now: u64) {
        match self {
            Self::Sbsa(uart) => uart.advance(now),
            Self::Ns16550(uart) => uart.advance(now),
        }
    }
    pub(super) fn deadline(&self) -> u64 {
        match self {
            Self::Sbsa(uart) => uart.next_timeout().unwrap_or(u64::MAX),
            Self::Ns16550(uart) => uart.next_timeout().unwrap_or(u64::MAX),
        }
    }
    pub(super) fn asserted(&self) -> bool {
        match self {
            Self::Sbsa(uart) => uart.interrupt_asserted(),
            Self::Ns16550(uart) => uart.interrupt_asserted(),
        }
    }
    pub(super) fn access(
        &mut self,
        offset: u64,
        request: vm::MmioRequest,
        now: u64,
    ) -> Result<(vm::MmioCompletion, Option<u8>)> {
        match self {
            Self::Sbsa(uart) => {
                let completion = match request.operation {
                    vm::MmioOperation::Read => vm::MmioCompletion::Read(
                        uart.read(offset, request.width as usize)
                            .map_err(|_| Error::InvalidResponse)?,
                    ),
                    vm::MmioOperation::Write(value) => {
                        uart.write(offset, request.width as usize, value, now)
                            .map_err(|_| Error::InvalidResponse)?;
                        vm::MmioCompletion::Write
                    }
                };
                Ok((completion, None))
            }
            Self::Ns16550(uart) => {
                if request.width != 1 || offset >= 8 {
                    return Err(Error::InvalidResponse);
                }
                Ok(match request.operation {
                    vm::MmioOperation::Read => (
                        vm::MmioCompletion::Read(u64::from(uart.read_at(offset as usize, now))),
                        None,
                    ),
                    vm::MmioOperation::Write(value) => (
                        vm::MmioCompletion::Write,
                        uart.write_at(offset as usize, value as u8, now),
                    ),
                })
            }
        }
    }
}
