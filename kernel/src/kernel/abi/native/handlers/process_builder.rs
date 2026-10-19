// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native process builder syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{NativeAction, ProcessBuilderServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_process_builder_service_error, status_only, success,
};
use crate::kernel::abi::native::wire::{
    parse_affinity_request, parse_builder_create, parse_builder_handle, parse_builder_text,
    require_zero,
};
use crate::kernel::capability::HandleValue;
use crate::kernel::capability::InvocationReceiver;
use hyper::abi::native::{
    HYPER_NATIVE_PROCESS_NAME_MAX_BYTES, HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_create(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_builder_create(arguments).and_then(|(group, domain, executable, loader)| {
        services
            .create_process_builder(receiver, group, domain, executable, loader)
            .map_err(status_from_process_builder_service_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_name(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result =
        parse_builder_text(arguments, HYPER_NATIVE_PROCESS_NAME_MAX_BYTES).and_then(|text| {
            services
                .set_process_builder_name(receiver, text)
                .map_err(status_from_process_builder_service_error)
        });
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_data(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_builder_text(arguments, HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES)
        .and_then(|text| {
            services
                .set_process_builder_data(receiver, text)
                .map_err(status_from_process_builder_service_error)
        });
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_affinity(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_affinity_request(arguments).and_then(|(words, word_count)| {
        services
            .set_process_builder_affinity(receiver, words, word_count)
            .map_err(status_from_process_builder_service_error)
    });
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_add_handle(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_builder_handle(arguments).and_then(
        |(source, purpose, expected_kind, requested_rights, operation)| {
            services
                .add_process_builder_handle(
                    receiver,
                    source,
                    purpose,
                    expected_kind,
                    requested_rights,
                    operation,
                )
                .map_err(status_from_process_builder_service_error)
        },
    );
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_seal(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    _arguments: &Arguments,
) -> NativeAction {
    let result = services
        .seal_process_builder(receiver)
        .map_err(status_from_process_builder_service_error);
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_start(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    _arguments: &Arguments,
) -> NativeAction {
    let result = services
        .start_process_builder(receiver)
        .map_err(status_from_process_builder_service_error);
    NativeAction::Return(match result {
        Ok(handles) => success(handles.map(HandleValue::get)),
        Err(status) => failure(status),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_abort(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    _arguments: &Arguments,
) -> NativeAction {
    let result = services
        .abort_process_builder(receiver)
        .map_err(status_from_process_builder_service_error);
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_process_builder_set_memory_lock(
    services: &impl ProcessBuilderServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        require_zero(&arguments[1..])?;
        let enabled = match arguments[0] {
            0 => false,
            1 => true,
            _ => return Err(hyper::abi::native::HYPER_NATIVE_STATUS_INVALID_ARGUMENT),
        };
        services
            .set_process_builder_memory_lock(receiver, enabled)
            .map_err(status_from_process_builder_service_error)
    })();
    NativeAction::Return(status_only(result))
}
