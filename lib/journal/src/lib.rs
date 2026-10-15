// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Journal registration is delegated only to supervisors. A service receives
//! byte writers; bytes cannot impersonate another source or forge metadata.

use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, HandleRef, OwnedHandle, Rights, RightsOffer,
};
use hyper_os::startup::StartupPurpose;
use serde::{Deserialize, Serialize};

pub const QUERY: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800b_0010);
pub const MIRROR: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x800b_0011);
pub const SOURCE_RIGHTS: Rights = Rights::READ.union(Rights::WAIT);

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Stream {
    Stdout,
    Stderr,
    Log,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub component: String,
    pub generation: u64,
    pub stream: Stream,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub sequence: u64,
    pub monotonic_ns: u64,
    pub source: Source,
    pub text: String,
    pub continued: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub component: Option<String>,
    pub after: u64,
    /// Inclusive upper cursor for a stable initial listing; None follows new records.
    pub through: Option<u64>,
    pub limit: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub records: Vec<Record>,
    pub next: u64,
    pub oldest: u64,
    pub newest: u64,
    pub discarded_records: u64,
    pub rejected_sources: u64,
    pub mirror_dropped_bytes: u64,
}

pub fn register(
    connector: HandleRef<'_, CapabilityChannelObject>,
    source: &Source,
    reader: OwnedHandle<ByteChannelObject>,
) -> hyper_os::Result<()> {
    if source.component.is_empty()
        || source.component.len() > 255
        || source.component.as_bytes().contains(&0)
    {
        return Err(hyper_os::Error::InvalidResponse);
    }
    let mut bytes = b"LOG2".to_vec();
    bytes.extend(serde_json::to_vec(source).map_err(|_| hyper_os::Error::InvalidResponse)?);
    let channel =
        CapabilityChannel::from_handle(connector.duplicate(hyper_service_ipc::CLIENT_RIGHTS)?);
    let mut reader = Some(reader);
    hyper_service_ipc::send(
        &channel,
        hyper_service_ipc::deadline()?,
        &bytes,
        &mut [CapabilityDisposition::move_handle(
            &mut reader,
            RightsOffer::Exact(SOURCE_RIGHTS),
        )?],
    )
}
