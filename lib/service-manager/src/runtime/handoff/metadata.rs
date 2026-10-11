// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable handoff description travels separately from live authority.

use super::super::Error;
use super::{
    Runtime, SystemInitConfig,
    inventory::{InventoryOffer, Slot},
};
use crate::manifest::MAX_MANIFEST_BYTES;
use hyper_os::handle::{OwnedHandle, TaskGroupObject, VmoObject};
use hyper_os::memory::{PAGE_SIZE, SnapshotVmo, WritableVmo};
use hyper_os::startup::{Startup, StartupPurpose};
use serde::{Deserialize, Serialize};
use std::io::Read;

pub(super) const SNAPSHOT: StartupPurpose<VmoObject> = StartupPurpose::new(0x8008_0002);
const MAX_OFFER_BYTES: usize = MAX_MANIFEST_BYTES * 2 + 16384;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparedOffer {
    version: u32,
    pub(super) core_manifest: String,
    pub(super) boot_failed: bool,
    pub(super) next: SystemInitConfig,
    pub(super) slots: Vec<Slot>,
}

impl PreparedOffer {
    pub(super) fn new(
        runtime: &Runtime,
        group: &OwnedHandle<TaskGroupObject>,
        next: &SystemInitConfig,
        core: &str,
    ) -> hyper_os::Result<Self> {
        let mut inventory = InventoryOffer::new(&runtime.launcher.authorities, group)?;
        inventory.describe_processes(&runtime.supervisors)?;
        Ok(Self {
            version: 1,
            core_manifest: core.into(),
            boot_failed: runtime.boot_failed,
            next: next.clone(),
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

    pub(super) fn read(startup: &mut Startup<'_>) -> hyper_os::Result<Self> {
        let handle = startup.take(SNAPSHOT)?;
        let snapshot = SnapshotVmo::from_vmo(handle.as_handle_ref())?;
        let mut length = [0; 8];
        snapshot.read_exact_at(0, &mut length)?;
        let length = usize::try_from(u64::from_le_bytes(length))
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
        if length > MAX_OFFER_BYTES {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let mut bytes = vec![0; length];
        snapshot.read_exact_at(8, &mut bytes)?;
        let offer: Self =
            serde_json::from_slice(&bytes).map_err(|_| hyper_os::Error::InvalidResponse)?;
        if offer.version != 1 || offer.core_manifest.len() > MAX_MANIFEST_BYTES {
            return Err(hyper_os::Error::InvalidResponse);
        }
        offer
            .next
            .validate()
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
        Ok(offer)
    }
}

pub(super) fn read_manifest(path: &str) -> Result<String, Error> {
    let file = std::fs::File::open(path).map_err(|error| {
        super::report_failure(&format!("opening service manifest {path}"), &error);
        Error::OperatingSystem
    })?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|error| {
            super::report_failure(&format!("reading service manifest {path}"), &error);
            Error::Source
        })?;
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(Error::Source);
    }
    Ok(text)
}
