// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit namespace administration; no boot layout is implied by these calls.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{NativeAction, VfsServices};
use crate::kernel::abi::native::status::{
    status_from_address_error, status_from_vfs_service_error, status_only,
};
use crate::kernel::abi::native::wire::{parse_handle, require_zero};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use hyper::abi::native as abi;

fn path(address: u64, length: u64) -> Result<UserSlice, abi::HyperNativeStatus> {
    if length == 0 || length > abi::HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES {
        return Err(abi::HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    UserSlice::new(UserAddress::new(address), length).map_err(status_from_address_error)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_filesystem_mount_ramfs(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        require_zero(&arguments[3..])?;
        services
            .mount_ramfs(
                parse_handle(arguments[0])?,
                path(arguments[1], arguments[2])?,
            )
            .map_err(status_from_vfs_service_error)
    })();
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_filesystem_switch_root(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let request = UserSlice::new(
            UserAddress::new(arguments[1]),
            abi::HYPER_NATIVE_FILESYSTEM_ROOT_SWITCH_MIN_SIZE as u64,
        )
        .map_err(status_from_address_error)?;
        services
            .switch_root(parse_handle(arguments[0])?, request)
            .map_err(status_from_vfs_service_error)
    })();
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_filesystem_begin_root_switch(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let request = UserSlice::new(
            UserAddress::new(arguments[1]),
            abi::HYPER_NATIVE_FILESYSTEM_ROOT_SWITCH_MIN_SIZE as u64,
        )
        .map_err(status_from_address_error)?;
        services
            .begin_root_switch(parse_handle(arguments[0])?, request)
            .map_err(status_from_vfs_service_error)
    })();
    NativeAction::Return(crate::kernel::abi::native::status::handle_result(result))
}
#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_filesystem_finish_root_switch(
    services: &impl VfsServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        require_zero(&arguments[2..])?;
        let commit = match arguments[1] {
            0 => false,
            1 => true,
            _ => return Err(abi::HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
        };
        services
            .finish_root_switch(parse_handle(arguments[0])?, commit)
            .map_err(status_from_vfs_service_error)
    })();
    NativeAction::Return(status_only(result))
}
