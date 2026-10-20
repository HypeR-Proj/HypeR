// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! PMU control validates authority before touching sampling state or user memory.
use crate::kernel::abi::native::{
    Arguments,
    services::{NativeAction, SystemInspectServices},
    status,
    wire::{copy_info_record, prepare_info_request},
};
use crate::kernel::capability::InvocationReceiver;

pub(in crate::kernel::abi::native) fn sys_cpu_inspector_profile_start(
    services: &impl SystemInspectServices,
    receiver: &InvocationReceiver,
    args: &Arguments,
) -> NativeAction {
    NativeAction::Return(status::info_result(
        services.profile_start(receiver, args[0], args[1]),
    ))
}
pub(in crate::kernel::abi::native) fn sys_cpu_inspector_profile_stop(
    services: &impl SystemInspectServices,
    receiver: &InvocationReceiver,
    args: &Arguments,
) -> NativeAction {
    NativeAction::Return(status::status_only(
        services.profile_stop(receiver, args[0]),
    ))
}
pub(in crate::kernel::abi::native) fn sys_cpu_inspector_profile_read(
    services: &impl SystemInspectServices,
    receiver: &InvocationReceiver,
    args: &Arguments,
) -> NativeAction {
    let result = (|| {
        let size = if args[1] == u64::MAX {
            crate::kernel::profile::INFO_SIZE
        } else {
            crate::kernel::profile::SAMPLE_SIZE
        };
        let request = prepare_info_request(&[args[3], args[4], 0, 0, 0, 0], size, size)?;
        let mut bytes = [0u8; crate::kernel::profile::SAMPLE_SIZE];
        let written = services.profile_read(receiver, args[0], args[1], args[2], &mut bytes)?;
        copy_info_record(services, request, &bytes[..written])
    })();
    NativeAction::Return(status::info_result(result))
}
