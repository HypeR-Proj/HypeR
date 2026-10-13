// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linear authority over a prepared, reversible namespace-root transition.
use super::{Error, RootTransition};
use crate::kernel::{
    accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind},
    authority::Rights,
    object::{
        KernelObject, ObjectKind, ObjectRetirement, TransferClass, object_allocation_size, private,
    },
};
use hyper::mm::FallibleArc;

pub(crate) struct RootTransitionObject {
    transition: FallibleArc<RootTransition>,
    _charge: CommittedCharge,
}

impl RootTransitionObject {
    pub(crate) fn try_new(
        transition: FallibleArc<RootTransition>,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let bytes = object_allocation_size::<Self>()
            .and_then(|bytes| bytes.checked_add(FallibleArc::<RootTransition>::allocation_size()))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::AllocationSize)?;
        let charge = sponsor
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, bytes),
            )?
            .commit();
        Ok(Self {
            transition,
            _charge: charge,
        })
    }
    pub(crate) fn finish(&self, commit: bool) -> Result<(), Error> {
        self.transition.finish(commit)
    }
}
impl private::Sealed for RootTransitionObject {}
impl private::UserExportable for RootTransitionObject {}
impl KernelObject for RootTransitionObject {
    const KIND: ObjectKind = ObjectKind::ROOT_TRANSITION;
    const SUPPORTED_RIGHTS: Rights = Rights::WRITE.union(Rights::TRANSFER).union(Rights::INSPECT);
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    fn on_zero_active_handles(&self, _: &mut ObjectRetirement) {
        self.transition.abort();
    }
}
