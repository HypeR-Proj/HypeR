// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native object syscall validation.

use crate::kernel::capability::handle_wire;

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{NativeAction, ObjectServiceError, ObjectServices};
use crate::kernel::abi::native::status::{
    failure, handle_result, status_from_address_error, status_from_object_service_error,
    status_only, success,
};
use crate::kernel::abi::native::wire::{parse_handle, parse_wait_many};
use crate::kernel::capability::InvocationReceiver;
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::object::{SignalWaitManyOutcome, SignalWaitOutcome};
use hyper::abi::native::{
    HYPER_NATIVE_STATUS_CANCELLED, HYPER_NATIVE_STATUS_INTERNAL,
    HYPER_NATIVE_STATUS_INVALID_ARGUMENT, HYPER_NATIVE_STATUS_TIMED_OUT, NativeResult,
};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_event_create(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    handle_result(
        services
            .create_event()
            .map_err(status_from_object_service_error),
    )
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_event_signal(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = services
        .signal_event(receiver, arguments[0], arguments[1])
        .map_err(status_from_object_service_error);
    NativeAction::Return(status_only(result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_wait_one(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = services
        .wait_one(receiver, arguments[0], arguments[1])
        .map_err(status_from_object_service_error);
    let result = match result {
        Ok(SignalWaitOutcome::Observed(snapshot)) => success([snapshot.signals().bits(), 0]),
        Ok(SignalWaitOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    NativeAction::Return(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_object_wait_many(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = parse_wait_many(arguments).and_then(|(items, item_count, deadline)| {
        services
            .wait_many(items, item_count, deadline)
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitManyOutcome::Observed { index, snapshot }) => match u64::try_from(index) {
            Ok(index) => success([index, snapshot.signals().bits()]),
            Err(_) => failure(HYPER_NATIVE_STATUS_INTERNAL),
        },
        Ok(SignalWaitManyOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitManyOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    NativeAction::Return(result)
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_create(
    services: &impl ObjectServices,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        if arguments[1..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_create(
            usize::try_from(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
        )
    })();
    NativeAction::Return(match result {
        Ok(value) => success([handle_wire::to_abi(value), 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_add(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        if arguments[2..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_add(
            receiver,
            parse_handle(arguments[0]).map_err(|_| ObjectServiceError::InvalidInput)?,
            arguments[1],
        )
    })();
    NativeAction::Return(match result {
        Ok(value) => success([value, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_rearm(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        if arguments[1..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_rearm(receiver, arguments[0])
    })();
    NativeAction::Return(match result {
        Ok(_) => success([0, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_remove(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        if arguments[1..].iter().any(|value| *value != 0) {
            return Err(ObjectServiceError::InvalidInput);
        }
        services.wait_set_remove(receiver, arguments[0])
    })();
    NativeAction::Return(match result {
        Ok(_) => success([0, 0]),
        Err(error) => failure(status_from_object_service_error(error)),
    })
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_wait_set_wait(
    services: &impl ObjectServices,
    receiver: &InvocationReceiver,
    arguments: &Arguments,
) -> NativeAction {
    let result = (|| {
        if arguments[2] != 24 || arguments[3] != 0 || arguments[4] != 0 {
            return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
        }
        let set = receiver;
        let output = UserSlice::new(UserAddress::new(arguments[1]), 24)
            .map_err(status_from_address_error)?;
        services
            .wait_set_wait(set, arguments[0], output)
            .map_err(status_from_object_service_error)
    })();
    NativeAction::Return(status_only(result))
}
