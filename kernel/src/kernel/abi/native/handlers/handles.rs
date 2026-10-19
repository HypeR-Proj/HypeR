// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native handles syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::HandleServices;
use crate::kernel::abi::native::status::{
    handle_result, info_result, status_from_process_error, status_only,
};
use crate::kernel::abi::native::wire::{
    HANDLE_INFO_SIZE, OBJECT_BASIC_INFO_SIZE, copy_info_record, encode_handle_info,
    encode_object_basic_info, prepare_info_request,
};
use crate::kernel::capability::InvocationReceiver;
use crate::kernel::capability::Rights;
use hyper::abi::native::{
    HYPER_NATIVE_HANDLE_INFO_MIN_SIZE, HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE, NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_handle_close(
    services: &impl HandleServices,
    receiver: &InvocationReceiver,
    _arguments: &Arguments,
) -> NativeResult {
    let result = services
        .close_handle(receiver)
        .map_err(status_from_process_error);
    status_only(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_handle_duplicate(
    services: &impl HandleServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeResult {
    let result = Rights::from_bits(arguments[0])
        .ok_or(hyper::abi::native::HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
        .and_then(|rights| {
            services
                .duplicate_handle(receiver, rights)
                .map_err(status_from_process_error)
        });
    handle_result(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_handle_replace(
    services: &impl HandleServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeResult {
    let result = Rights::from_bits(arguments[0])
        .ok_or(hyper::abi::native::HYPER_NATIVE_STATUS_INVALID_ARGUMENT)
        .and_then(|rights| {
            services
                .replace_handle(receiver, rights)
                .map_err(status_from_process_error)
        });
    handle_result(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_handle_get_info(
    services: &(impl HandleServices + crate::kernel::abi::native::services::UserMemoryServices),
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeResult {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_HANDLE_INFO_MIN_SIZE,
        HANDLE_INFO_SIZE,
    )
    .and_then(|request| {
        let value = receiver;
        let info = services
            .handle_info(value, Rights::NONE)
            .map_err(status_from_process_error)?;
        let record = encode_handle_info(info);
        copy_info_record(services, request, &record)
    });
    info_result(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_get_basic_info(
    services: &(impl HandleServices + crate::kernel::abi::native::services::UserMemoryServices),
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeResult {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE,
        OBJECT_BASIC_INFO_SIZE,
    )
    .and_then(|request| {
        let value = receiver;
        let info = services
            .handle_info(value, Rights::INSPECT)
            .map_err(status_from_process_error)?;
        let record = encode_object_basic_info(info);
        copy_info_record(services, request, &record)
    });
    info_result(result)
}
