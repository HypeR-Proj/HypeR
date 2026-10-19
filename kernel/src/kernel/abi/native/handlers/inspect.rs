// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native inspect syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{InspectServices, NativeAction, SystemInspectServices};
use crate::kernel::abi::native::status::{
    handle_result, info_result, scan_result, status_from_inspection_error,
};
use crate::kernel::abi::native::wire::{
    copy_encoded_page, copy_info_record, encode_cpu_observation, encode_handle_inspection,
    encode_memory_observation, encode_object_inspection, encode_task_process, encode_task_thread,
    parse_handle, parse_handle_inspector_scan, parse_inspector_scan, prepare_info_request,
};
use crate::kernel::capability::InvocationReceiver;
use crate::kernel::inspect::{OBJECT_PAGE_CAPACITY, Page};
use hyper::abi::native::{
    HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE, HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
    HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE, HyperNativeCpuObservation, HyperNativeMemoryObservation,
    HyperNativeObjectDetails, HyperNativeObjectInspection, HyperNativeTaskProcess,
    HyperNativeTaskThread,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_read_details(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        let request = prepare_info_request(
            &[arguments[3], arguments[4], 0, 0, 0, 0],
            HYPER_NATIVE_OBJECT_DETAILS_MIN_SIZE,
            core::mem::size_of::<HyperNativeObjectDetails>(),
        )?;
        let value = services
            .read_object_details(receiver, arguments[0], arguments[1], arguments[2])
            .map_err(status_from_inspection_error)?;
        let record = crate::kernel::abi::native::wire::encode_object_details(&value);
        copy_info_record(services, request, &record)
    })();
    NativeAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_scan_processes(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::PROCESS_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskProcess>(),
    )
    .and_then(|(cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_processes(receiver, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_process)
    });
    NativeAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_scan_threads(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_inspector_scan(
        arguments,
        crate::kernel::inspect::THREAD_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeTaskThread>(),
    )
    .and_then(|(cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_threads(receiver, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_task_thread)
    });
    NativeAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_scan_objects(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_inspector_scan(
        arguments,
        OBJECT_PAGE_CAPACITY,
        core::mem::size_of::<HyperNativeObjectInspection>(),
    )
    .and_then(|(cursor, destination)| {
        let mut page = Page::empty();
        services
            .scan_objects(receiver, cursor, &mut page)
            .map_err(status_from_inspection_error)?;
        copy_encoded_page(services, destination, &page, encode_object_inspection)
    });
    NativeAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_scan_handles(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result =
        parse_handle_inspector_scan(arguments).and_then(|(process_koid, cursor, destination)| {
            let page = services
                .scan_process_handles(receiver, process_koid, cursor)
                .map_err(status_from_inspection_error)?;
            copy_encoded_page(services, destination, &page, encode_handle_inspection)
        });
    NativeAction::Return(scan_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_process(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|process| {
        services
            .derive_task_inspector(receiver, process)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_process(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|process| {
        services
            .derive_object_inspector(receiver, process)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_task_group(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|group| {
        services
            .derive_task_inspector_for_task_group(receiver, group)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_task_group(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|group| {
        services
            .derive_object_inspector_for_task_group(receiver, group)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_task_inspector_derive_resource_domain(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|domain| {
        services
            .derive_task_inspector_for_resource_domain(receiver, domain)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_inspector_derive_resource_domain(
    services: &impl InspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_handle(arguments[0]).and_then(|domain| {
        services
            .derive_object_inspector_for_resource_domain(receiver, domain)
            .map_err(status_from_inspection_error)
    });
    NativeAction::Return(handle_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_memory_inspector_read(
    services: &impl SystemInspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeMemoryObservation>(),
    )
    .and_then(|request| {
        let inspector = receiver;
        let observation = services
            .memory_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_memory_observation(observation))
    });
    NativeAction::Return(info_result(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_cpu_inspector_read(
    services: &impl SystemInspectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = prepare_info_request(
        arguments,
        HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE,
        core::mem::size_of::<HyperNativeCpuObservation>(),
    )
    .and_then(|request| {
        let inspector = receiver;
        let observation = services
            .cpu_observation(inspector)
            .map_err(status_from_inspection_error)?;
        copy_info_record(services, request, &encode_cpu_observation(observation))
    });
    NativeAction::Return(info_result(result))
}
