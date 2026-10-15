// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Component-local connections and lifecycle observation. The private connector
//! authenticates the calling instance; a supplied name never grants authority.

pub mod connection;
pub mod installation;
pub mod objects;

use hyper_os::{handle::CapabilityChannelObject, startup::StartupPurpose};
use serde::{Deserialize, Serialize};

pub const CONNECTOR: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800c_0001);
pub const OUTGOING: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800c_0002);
pub const OBSERVE: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800c_0003);
pub const CONTROL: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x800c_0004);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Stopped,
    Starting,
    Running,
    Stopping,
    RestartWait,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentStatus {
    pub path: String,
    pub state: State,
    pub generation: u64,
    pub process: Option<u64>,
    pub restarts: u32,
    pub detail: String,
    pub published: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub captured_at_ns: u64,
    pub components: Vec<ComponentStatus>,
    pub progress: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Status {
        path: Option<String>,
    },
    Routes {
        path: String,
        capability: Option<String>,
    },
    Start {
        path: String,
    },
    Stop {
        path: String,
    },
    Restart {
        path: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub error: Option<String>,
    pub snapshot: Snapshot,
    pub routes: Vec<hyper_component_config::Route>,
}

/// One-shot readiness for a startup transaction. The coordinator may close the
/// receiver after READY; ongoing status uses the instance connector instead.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Notification {
    Ready,
}

pub fn notify(
    startup: &hyper_os::startup::Startup<'_>,
    event: &Notification,
) -> hyper_os::Result<()> {
    let channel = startup.borrow(NOTIFY)?;
    let bytes = serde_json::to_vec(event).map_err(|_| hyper_os::Error::InvalidResponse)?;
    if bytes.len() > 1024 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    channel
        .duplicate(hyper_os::handle::Rights::WRITE.union(hyper_os::handle::Rights::WAIT))?
        .as_byte_channel()
        .send(&bytes)
}

pub const NOTIFY: hyper_os::startup::StartupPurpose<hyper_os::handle::ByteChannelObject> =
    hyper_os::startup::StartupPurpose::new(0x800c_0005);
