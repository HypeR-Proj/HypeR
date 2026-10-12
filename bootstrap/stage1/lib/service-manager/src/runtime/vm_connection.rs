// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Reserve the console's VM control connection without creating a VM fleet.

use hyper_os::capability_channel::CapabilityChannel;
use hyper_os::handle::{CapabilityChannelObject, OwnedHandle};

/// A shell may receive a connection before the system manager exists. Keeping
/// this rendezvous separate does not allocate a fleet or require VM authority.
pub(super) struct VmManagerConnection {
    pub(super) client: OwnedHandle<CapabilityChannelObject>,
    pub(super) server: Option<OwnedHandle<CapabilityChannelObject>>,
}

impl VmManagerConnection {
    pub(super) fn new() -> hyper_os::Result<Self> {
        let (client, server) = CapabilityChannel::create()?;
        Ok(Self {
            client: client.into_handle(),
            server: Some(server.into_handle()),
        })
    }
}
