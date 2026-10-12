// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exact typed inventory. Private wire keys name roles, never sender handle IDs.

use super::super::{authority::AuthorityInventory, supervisor::SupervisorSet};
use crate::manifest::MAX_SERVICES;
use hyper_os::capability_channel::CapabilityDisposition;
use hyper_os::handle::*;
use serde::{Deserialize, Serialize};

const ROOT_DIRECTORY: u16 = 1;
const LIBRARY_DIRECTORY: u16 = 2;
const FACTORY: u16 = 3;
const GROUP: u16 = 4;
const DOMAIN: u16 = 5;
const TASK_INSPECTOR: u16 = 6;
const OBJECT_INSPECTOR: u16 = 7;
const MEMORY_INSPECTOR: u16 = 8;
const CPU_INSPECTOR: u16 = 9;
const SERVICE_OUTPUT: u16 = 10;
const DEVICE: u16 = 11;
const CONSOLE_INPUT: u16 = 12;
const CONSOLE_OUTPUT: u16 = 13;
const SESSION_INPUT: u16 = 14;
const SESSION_OUTPUT: u16 = 15;
const IO_BROKER_SERVER: u16 = 16;
const IO_BROKER_CLIENT: u16 = 17;
const IO_READY: u16 = 18;
const FILESYSTEM_ATTACH: u16 = 19;
const FILESYSTEM_PROVIDER: u16 = 20;
const FILESYSTEM_CONTROL_SERVER: u16 = 21;
const FILESYSTEM_CONTROL_CLIENT: u16 = 22;
const VM_AUTHORITY: u16 = 25;
const VM_CLIENT: u16 = 27;
const VM_MANAGER: u16 = 28;
const STAGE_GROUP: u16 = 30;
const CONSOLE: u16 = 31;
const BOOT_ENVIRONMENT: u16 = 32;
const SYSTEM_ENVIRONMENT: u16 = 33;
const SESSION_STARTUP: u16 = 34;
const SESSION_DECISION: u16 = 35;

const PROCESS_BASE: u16 = 100;
const MAX_SLOTS: usize = 64;
const _: () = assert!(SESSION_DECISION as usize + MAX_SERVICES <= MAX_SLOTS);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Slot {
    key: u16,
    rights: u64,
}

pub(super) struct InventoryOffer<'a> {
    pub(super) slots: Vec<Slot>,
    pub(super) dispositions: Vec<CapabilityDisposition<'a>>,
}

impl<'a> InventoryOffer<'a> {
    pub(super) fn new(
        a: &'a AuthorityInventory,
        group: &'a OwnedHandle<TaskGroupObject>,
    ) -> hyper_os::Result<Self> {
        let mut offer = Self {
            slots: Vec::with_capacity(MAX_SLOTS),
            dispositions: Vec::with_capacity(MAX_SLOTS),
        };
        offer.add(ROOT_DIRECTORY, a.root_directory.as_handle_ref())?;
        offer.add(LIBRARY_DIRECTORY, a.library_directory.as_handle_ref())?;
        offer.add(BOOT_ENVIRONMENT, a.boot_environment.as_handle_ref())?;
        offer.add(
            SYSTEM_ENVIRONMENT,
            a.system_environment
                .as_ref()
                .ok_or(hyper_os::Error::MissingHandle)?
                .as_handle_ref(),
        )?;
        offer.add(FACTORY, a.factory.as_handle_ref())?;
        offer.add(GROUP, a.group.as_handle_ref())?;
        offer.add(DOMAIN, a.domain.as_handle_ref())?;
        offer.add(TASK_INSPECTOR, a.task_inspector.as_handle_ref())?;
        offer.add(OBJECT_INSPECTOR, a.object_inspector.as_handle_ref())?;
        offer.add(MEMORY_INSPECTOR, a.memory_inspector.as_handle_ref())?;
        offer.add(CPU_INSPECTOR, a.cpu_inspector.as_handle_ref())?;
        offer.add(SERVICE_OUTPUT, a.service_output_channel.as_handle_ref())?;
        if let Some(handle) = &a.device {
            offer.add(DEVICE, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.console_input_channel {
            offer.add(CONSOLE_INPUT, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.console_output_channel {
            offer.add(CONSOLE_OUTPUT, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.session_input_channel {
            offer.add(SESSION_INPUT, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.session_output_channel {
            offer.add(SESSION_OUTPUT, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.session_startup_channel {
            offer.add(SESSION_STARTUP, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.session_startup_writer {
            offer.add(SESSION_DECISION, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.io_broker_server {
            offer.add(IO_BROKER_SERVER, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.io_broker_client {
            offer.add(IO_BROKER_CLIENT, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.io_ready_channel {
            offer.add(IO_READY, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.filesystem_attach {
            offer.add(FILESYSTEM_ATTACH, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.filesystem_provider {
            offer.add(FILESYSTEM_PROVIDER, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.filesystem_control_server {
            offer.add(FILESYSTEM_CONTROL_SERVER, handle.as_handle_ref())?;
        }
        if let Some(handle) = &a.filesystem_control_client {
            offer.add(FILESYSTEM_CONTROL_CLIENT, handle.as_handle_ref())?;
        }
        if let Some(authority) = &a.vm_creation {
            offer.add(VM_AUTHORITY, authority.as_handle_ref())?;
        }
        if let Some(connection) = &a.vm_connection {
            offer.add(VM_CLIENT, connection.client.as_handle_ref())?;
            if let Some(handle) = &connection.server {
                offer.add(VM_MANAGER, handle.as_handle_ref())?;
            }
        }
        offer.add(STAGE_GROUP, group.as_handle_ref())?;
        offer.add(CONSOLE, a.console.as_handle_ref())?;
        Ok(offer)
    }

    fn add<T: TypedObject>(&mut self, key: u16, source: HandleRef<'a, T>) -> hyper_os::Result<()> {
        let rights = source.info()?.rights;
        self.dispositions.push(CapabilityDisposition::duplicate(
            source,
            RightsOffer::Exact(rights),
        )?);
        self.slots.push(Slot {
            key,
            rights: rights.bits(),
        });
        Ok(())
    }

    pub(super) fn describe_processes(
        &mut self,
        supervisors: &SupervisorSet,
    ) -> hyper_os::Result<()> {
        for (index, process) in supervisors.processes.iter().enumerate() {
            if let Some(process) = process {
                self.slots.push(Slot {
                    key: PROCESS_BASE + index as u16,
                    rights: process.info()?.rights.bits(),
                });
            }
        }
        Ok(())
    }
}
