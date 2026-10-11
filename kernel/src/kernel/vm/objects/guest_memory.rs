// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit authority to share frozen writable backing among guest address spaces.

use super::{Error, reserve_object_charge};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::mm::user_space::{GuestMemoryBacking, VmoObject};
use crate::kernel::object::{KernelObject, ObjectKind, TransferClass, private};
use core::sync::atomic::{AtomicUsize, Ordering};

const DEDICATED_INITIATOR: usize = usize::MAX;

pub(crate) struct GuestMemoryObject {
    backing: GuestMemoryBacking,
    // Zero is available; positive counts retain active Native copies. The
    // terminal value permanently dedicates backing to a kernel block owner.
    native_access: AtomicUsize,
    _charge: CommittedCharge,
}

impl GuestMemoryObject {
    pub(crate) fn try_new(vmo: &VmoObject, domain: &ResourceDomain) -> Result<Self, Error> {
        let charge = reserve_object_charge::<Self>(domain)?;
        Ok(Self {
            backing: GuestMemoryBacking::try_from_vmo(vmo)?,
            native_access: AtomicUsize::new(0),
            _charge: charge,
        })
    }
    pub(crate) fn claim_native_initiator(&self) -> bool {
        self.native_access
            .compare_exchange(0, DEDICATED_INITIATOR, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub(crate) fn begin_native_access(&self) -> Option<NativeAccess<'_>> {
        self.native_access
            .fetch_update(Ordering::Acquire, Ordering::Relaxed, |state| {
                (state < DEDICATED_INITIATOR - 1).then(|| state + 1)
            })
            .ok()
            .map(|_| NativeAccess { memory: self })
    }
    pub(crate) fn backing(&self) -> GuestMemoryBacking {
        self.backing.clone()
    }
}

/// The resolving handle keeps the object alive; this admission excludes the
/// permanent kernel-initiator transition without holding a lock over copyout.
pub(crate) struct NativeAccess<'a> {
    memory: &'a GuestMemoryObject,
}
impl NativeAccess<'_> {
    pub(crate) fn backing(&self) -> &GuestMemoryBacking {
        &self.memory.backing
    }
}
impl Drop for NativeAccess<'_> {
    fn drop(&mut self) {
        self.memory.native_access.fetch_sub(1, Ordering::Release);
    }
}
impl private::Sealed for GuestMemoryObject {}
impl private::UserExportable for GuestMemoryObject {}
impl KernelObject for GuestMemoryObject {
    const KIND: ObjectKind = ObjectKind::GUEST_MEMORY;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::DUPLICATE)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::MAP)
        .union(Rights::INSPECT);
}
