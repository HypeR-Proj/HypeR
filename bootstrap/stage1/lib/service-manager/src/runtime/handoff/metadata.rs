// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable handoff description travels separately from live authority.

use super::{
    Runtime,
    inventory::{InventoryOffer, Slot},
};
use crate::manifest::MAX_MANIFEST_BYTES;
use hyper_os::handle::{OwnedHandle, TaskGroupObject, VmoObject};
use hyper_os::memory::{PAGE_SIZE, SnapshotVmo, WritableVmo};
use hyper_os::startup::StartupPurpose;
use serde::{Deserialize, Serialize};

pub(super) const SNAPSHOT: StartupPurpose<VmoObject> = StartupPurpose::new(0x8008_0002);
const MAX_OFFER_BYTES: usize = MAX_MANIFEST_BYTES * 2 + 16384;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedOffer {
    version: u32,
    pub(super) core_manifest: String,
    pub(super) storage_ready: bool,
    pub(super) slots: Vec<Slot>,
}

impl PreparedOffer {
    pub(super) fn new(
        runtime: &Runtime,
        group: &OwnedHandle<TaskGroupObject>,
        core: &str,
    ) -> hyper_os::Result<Self> {
        let mut inventory = InventoryOffer::new(&runtime.launcher.authorities, group)?;
        inventory.describe_processes(&runtime.supervisors)?;
        Ok(Self {
            version: 1,
            core_manifest: core.into(),
            storage_ready: runtime.storage_ready,
            slots: inventory.slots,
        })
    }

    pub(super) fn snapshot(&self) -> hyper_os::Result<SnapshotVmo> {
        let bytes = serde_json::to_vec(self).map_err(|_| hyper_os::Error::InvalidResponse)?;
        if bytes.len() > MAX_OFFER_BYTES {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let size = (bytes.len() as u64 + 8).div_ceil(PAGE_SIZE) * PAGE_SIZE;
        let vmo = WritableVmo::create(size)?;
        vmo.write_all_at(0, &(bytes.len() as u64).to_le_bytes())?;
        vmo.write_all_at(8, &bytes)?;
        SnapshotVmo::from_vmo(vmo.as_handle_ref())
    }
}
