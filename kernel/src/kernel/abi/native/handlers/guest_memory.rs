// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native backend copies through explicit guest-memory authority.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{DeferredAction, VmServices};
use crate::kernel::abi::native::status::{status_from_vm_service_error, status_only};
use crate::kernel::abi::native::wire::{optional_user_slice, parse_handle, require_zero};
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::UserSlice;
use hyper::abi::native::{
    HYPER_NATIVE_GUEST_MEMORY_MAX_TRANSFER_BYTES, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HyperNativeStatus,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_memory_read(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_transfer(arguments).and_then(|(memory, offset, bytes)| {
        services
            .read_guest_memory(memory, offset, bytes)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_guest_memory_write(
    services: &impl VmServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_transfer(arguments).and_then(|(memory, offset, bytes)| {
        services
            .write_guest_memory(memory, offset, bytes)
            .map_err(status_from_vm_service_error)
    });
    DeferredAction::Return(status_only(result))
}

fn parse_transfer(
    arguments: &Arguments,
) -> Result<(HandleValue, u64, Option<UserSlice>), HyperNativeStatus> {
    require_zero(&arguments[4..])?;
    if arguments[3] > HYPER_NATIVE_GUEST_MEMORY_MAX_TRANSFER_BYTES {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    Ok((
        parse_handle(arguments[0])?,
        arguments[1],
        optional_user_slice(arguments[2], arguments[3])?,
    ))
}
