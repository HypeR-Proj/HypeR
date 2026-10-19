// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native console syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{ConsoleServices, NativeAction};
use crate::kernel::abi::native::status::{console_io_result, status_from_console_service_error};
use crate::kernel::abi::native::wire::{parse_console_io, require_zero};
use crate::kernel::capability::InvocationReceiver;

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_console_discard_input(
    services: &impl ConsoleServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = require_zero(arguments).and_then(|()| {
        services
            .discard_console_input(receiver)
            .map_err(status_from_console_service_error)
    });
    NativeAction::Return(console_io_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_console_read(
    services: &impl ConsoleServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_console_io(arguments).and_then(|bytes| {
        services
            .read_console(receiver, bytes)
            .map_err(status_from_console_service_error)
    });
    NativeAction::Return(console_io_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_console_write(
    services: &impl ConsoleServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_console_io(arguments).and_then(|bytes| {
        services
            .write_console(receiver, bytes)
            .map_err(status_from_console_service_error)
    });
    NativeAction::Return(console_io_result(result))
}
