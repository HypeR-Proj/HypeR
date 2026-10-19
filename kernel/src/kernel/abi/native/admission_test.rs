// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admission tests use real generation handles independently of payload mocks.

use core::cell::{Cell, RefCell};

use hyper::abi::native::{self as abi, NativeInvocation, NativeOperation};

use crate::kernel::accounting::{ResourceDomain, ResourceLimits};
use crate::kernel::capability::{
    HandleError, HandleFlags, HandleInfo, HandleTable, HandleTableStoragePlan, HandleValue,
    InvocationReceiver, PreparedHandle, Rights,
};
use crate::kernel::object::{Event, PublishableRef};
use crate::kernel::process::ProcessError;

use super::dispatch::admit_invocation;
use super::services::HandleServices;

struct AdmissionServices {
    table: RefCell<HandleTable>,
    context_allowed: Cell<bool>,
    handles: [HandleValue; 2],
}

impl Drop for AdmissionServices {
    fn drop(&mut self) {
        for handle in self.handles {
            if let Ok(closed) = self.table.get_mut().remove(handle) {
                closed.complete();
            }
        }
    }
}

impl HandleServices for AdmissionServices {
    fn resolve_invocation(&self, value: HandleValue) -> Result<InvocationReceiver, ProcessError> {
        Ok(self.table.borrow().resolve_invocation(value)?)
    }

    fn validate_invocation_context(
        &self,
        _receiver: &InvocationReceiver,
    ) -> Result<(), ProcessError> {
        if self.context_allowed.get() {
            Ok(())
        } else {
            Err(HandleError::AccessDenied.into())
        }
    }

    fn close_handle(&self, receiver: &InvocationReceiver) -> Result<(), ProcessError> {
        self.table
            .borrow_mut()
            .remove(receiver.handle())?
            .complete();
        Ok(())
    }

    fn handle_info(
        &self,
        receiver: &InvocationReceiver,
        rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        receiver.require(rights)?;
        Ok(receiver.info())
    }

    fn duplicate_handle(
        &self,
        _receiver: &InvocationReceiver,
        _rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        Err(HandleError::AccessDenied.into())
    }

    fn replace_handle(
        &self,
        _receiver: &InvocationReceiver,
        _rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        Err(HandleError::AccessDenied.into())
    }
}

pub(super) fn run() -> Result<(), ()> {
    let domain = ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(|_| ())?;
    let prepare = |rights| {
        let event =
            PublishableRef::try_new(Event::try_new(&domain).map_err(|_| ())?).map_err(|_| ())?;
        PreparedHandle::try_from_new_object(event.publication(), rights, HandleFlags::NONE)
            .map_err(|_| ())
    };
    let prepared = [prepare(Rights::SIGNAL)?, prepare(Rights::INSPECT)?];
    let mut table = HandleTable::new();
    let storage = table.reservation_storage_snapshot_for(2).map_err(|_| ())?;
    let mut storage = Some(HandleTableStoragePlan::try_new(storage).map_err(|_| ())?);
    let reservation = table.reserve_with_plan::<2>(&mut storage).map_err(|_| ())?;
    let [signal, inspect] = reservation.publish(&mut table, prepared);
    let services = AdmissionServices {
        table: RefCell::new(table),
        context_allowed: Cell::new(true),
        handles: [signal, inspect],
    };
    let invocation = |target, operation| NativeInvocation::new(target, operation, [0; 6], 0);
    let admitted = |target, operation| admit_invocation(&services, &invocation(target, operation));
    let fails = |target, operation, expected| matches!(admitted(target, operation), Err(status) if status == expected);

    // A method selector has meaning only within the receiver's concrete kind.
    // EventSignal and AbiQuery intentionally share a numeric selector.
    if NativeOperation::AbiQuery.operation() != NativeOperation::EventSignal.operation()
        || !matches!(
            admitted(signal.get(), NativeOperation::AbiQuery.operation()),
            Ok((_, NativeOperation::EventSignal))
        )
        || !fails(
            inspect.get(),
            NativeOperation::EventSignal.operation(),
            abi::HYPER_NATIVE_STATUS_ACCESS_DENIED,
        )
        || !fails(
            0,
            NativeOperation::EventSignal.operation(),
            abi::HYPER_NATIVE_STATUS_BAD_HANDLE,
        )
        || !fails(
            u64::MAX,
            NativeOperation::EventSignal.operation(),
            abi::HYPER_NATIVE_STATUS_BAD_HANDLE,
        )
        || !fails(
            signal.get(),
            u64::MAX,
            abi::HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        )
    {
        return Err(());
    }

    // Context validation must not be skipped after kind and rights admission.
    services.context_allowed.set(false);
    if !fails(
        signal.get(),
        NativeOperation::EventSignal.operation(),
        abi::HYPER_NATIVE_STATUS_ACCESS_DENIED,
    ) {
        return Err(());
    }
    services.context_allowed.set(true);
    services
        .table
        .borrow_mut()
        .remove(signal)
        .map_err(|_| ())?
        .complete();
    if !fails(
        signal.get(),
        NativeOperation::EventSignal.operation(),
        abi::HYPER_NATIVE_STATUS_BAD_HANDLE,
    ) {
        return Err(());
    }
    services
        .table
        .borrow_mut()
        .remove(inspect)
        .map_err(|_| ())?
        .complete();
    Ok(())
}
