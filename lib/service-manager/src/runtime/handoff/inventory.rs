// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exact typed inventory. Private wire keys name roles, never sender handle IDs.

use super::super::{
    Runtime,
    authority::AuthorityInventory,
    fleet::{VmFleet, VmManagerConnection},
    launcher::ServiceLauncher,
    provision::FleetProvisioner,
    supervisor::SupervisorSet,
};
use crate::manifest::MAX_SERVICES;
use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::fs::Directory;
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
const VM_GROUP: u16 = 23;
const VM_DOMAIN: u16 = 24;
const VM_AUTHORITY: u16 = 25;
const VM_PROVISIONING: u16 = 26;
const VM_CLIENT: u16 = 27;
const VM_MANAGER: u16 = 28;
const PROVISIONER: u16 = 29;
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
        if let Some(fleet) = &a.vm_fleet {
            offer.add(VM_GROUP, fleet.group.as_handle_ref())?;
            offer.add(VM_DOMAIN, fleet.domain.as_handle_ref())?;
            if let Some(handle) = &fleet.provisioning {
                offer.add(VM_PROVISIONING, handle.as_handle_ref())?;
            }
            if let Some(provisioner) = &fleet.provisioner {
                offer.add(PROVISIONER, provisioner.channel.as_handle_ref())?;
            }
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

fn receive_slot(slot: &Slot) -> hyper_os::Result<CapabilityReceiveSlot> {
    let rights = Rights::from_bits(slot.rights).ok_or(hyper_os::Error::InvalidResponse)?;
    Ok(match slot.key {
        ROOT_DIRECTORY | BOOT_ENVIRONMENT | SYSTEM_ENVIRONMENT => {
            CapabilityReceiveSlot::new::<DirectoryObject>(rights)
        }
        LIBRARY_DIRECTORY => CapabilityReceiveSlot::new::<DirectoryObject>(rights),
        FACTORY => CapabilityReceiveSlot::new::<TaskFactoryObject>(rights),
        GROUP => CapabilityReceiveSlot::new::<TaskGroupObject>(rights),
        DOMAIN => CapabilityReceiveSlot::new::<ResourceDomainObject>(rights),
        TASK_INSPECTOR => CapabilityReceiveSlot::new::<TaskInspectorObject>(rights),
        OBJECT_INSPECTOR => CapabilityReceiveSlot::new::<ObjectInspectorObject>(rights),
        MEMORY_INSPECTOR => CapabilityReceiveSlot::new::<MemoryInspectorObject>(rights),
        CPU_INSPECTOR => CapabilityReceiveSlot::new::<CpuInspectorObject>(rights),
        SERVICE_OUTPUT => CapabilityReceiveSlot::new::<ByteChannelObject>(rights),
        DEVICE => CapabilityReceiveSlot::new::<DeviceAssignmentAuthorityObject>(rights),
        CONSOLE_INPUT => CapabilityReceiveSlot::new::<ByteChannelObject>(rights),
        CONSOLE_OUTPUT => CapabilityReceiveSlot::new::<ByteChannelObject>(rights),
        SESSION_INPUT => CapabilityReceiveSlot::new::<ByteChannelObject>(rights),
        SESSION_OUTPUT | SESSION_STARTUP | SESSION_DECISION => {
            CapabilityReceiveSlot::new::<ByteChannelObject>(rights)
        }
        IO_BROKER_SERVER => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        IO_BROKER_CLIENT => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        IO_READY => CapabilityReceiveSlot::new::<ByteChannelObject>(rights),
        FILESYSTEM_ATTACH => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        FILESYSTEM_PROVIDER => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        FILESYSTEM_CONTROL_SERVER => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        FILESYSTEM_CONTROL_CLIENT => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        VM_GROUP => CapabilityReceiveSlot::new::<TaskGroupObject>(rights),
        VM_DOMAIN => CapabilityReceiveSlot::new::<ResourceDomainObject>(rights),
        VM_AUTHORITY => CapabilityReceiveSlot::new::<VirtualMachineCreationAuthorityObject>(rights),
        VM_PROVISIONING => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        VM_CLIENT => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        VM_MANAGER => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        PROVISIONER => CapabilityReceiveSlot::new::<CapabilityChannelObject>(rights),
        STAGE_GROUP => CapabilityReceiveSlot::new::<TaskGroupObject>(rights),
        CONSOLE => CapabilityReceiveSlot::new::<ConsoleObject>(rights),
        key if (PROCESS_BASE..PROCESS_BASE + MAX_SERVICES as u16).contains(&key) => {
            CapabilityReceiveSlot::new::<ProcessObject>(rights)
        }
        _ => return Err(hyper_os::Error::InvalidResponse),
    })
}

/// Validates all role/type/shape invariants before registration can accept MOVE.
fn validate_inventory(slots: &[Slot], core_count: usize) -> hyper_os::Result<()> {
    if slots.len() > MAX_SLOTS || core_count > MAX_SERVICES {
        return Err(hyper_os::Error::InvalidResponse);
    }
    for (index, slot) in slots.iter().enumerate() {
        if !(ROOT_DIRECTORY..=SESSION_DECISION).contains(&slot.key)
            && !(PROCESS_BASE..PROCESS_BASE + MAX_SERVICES as u16).contains(&slot.key)
        {
            return Err(hyper_os::Error::InvalidResponse);
        }
        if Rights::from_bits(slot.rights).is_none() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        if slots[..index].iter().any(|s| s.key == slot.key)
            || (slot.key >= PROCESS_BASE && usize::from(slot.key - PROCESS_BASE) >= core_count)
        {
            return Err(hyper_os::Error::InvalidResponse);
        }
    }
    for required in [
        BOOT_ENVIRONMENT,
        SYSTEM_ENVIRONMENT,
        ROOT_DIRECTORY,
        LIBRARY_DIRECTORY,
        FACTORY,
        GROUP,
        DOMAIN,
        TASK_INSPECTOR,
        OBJECT_INSPECTOR,
        MEMORY_INSPECTOR,
        CPU_INSPECTOR,
        SERVICE_OUTPUT,
        STAGE_GROUP,
        CONSOLE,
    ] {
        if !slots.iter().any(|s| s.key == required) {
            return Err(hyper_os::Error::InvalidResponse);
        }
    }
    for key in [BOOT_ENVIRONMENT, SYSTEM_ENVIRONMENT] {
        let slot = slots
            .iter()
            .find(|slot| slot.key == key)
            .ok_or(hyper_os::Error::InvalidResponse)?;
        let rights = Rights::from_bits(slot.rights).ok_or(hyper_os::Error::InvalidResponse)?;
        if !rights
            .contains(hyper_service::process::ENVIRONMENT_DIRECTORY_CONTRACT.required_rights())
        {
            return Err(hyper_os::Error::MissingHandle);
        }
    }
    let has = |key| slots.iter().any(|slot| slot.key == key);
    if has(VM_GROUP) != has(VM_DOMAIN)
        || (!has(VM_GROUP) && (has(VM_PROVISIONING) || has(PROVISIONER)))
        || (has(VM_MANAGER) && !has(VM_CLIENT))
    {
        return Err(hyper_os::Error::InvalidResponse);
    }
    Ok(())
}

/// A still-waiting core session must receive its decision from the adopted
/// supervisor. Reject a handoff that would lose the sole decision writer.
pub(super) fn validate_session_source(
    slots: &[Slot],
    plan: &crate::manifest::LaunchPlan<'_>,
    core_count: usize,
) -> hyper_os::Result<()> {
    if plan
        .unique_service_for_purpose(hyper_service::session::STARTUP_DECISION.as_raw())
        .is_some_and(|index| index < core_count)
    {
        let slot = slots
            .iter()
            .find(|slot| slot.key == SESSION_DECISION)
            .ok_or(hyper_os::Error::MissingHandle)?;
        let rights = Rights::from_bits(slot.rights).ok_or(hyper_os::Error::InvalidResponse)?;
        if !rights.contains(Rights::WRITE) {
            return Err(hyper_os::Error::MissingHandle);
        }
    }
    Ok(())
}

/// Reject missing VM authority before accepting core process ownership. Fleet
/// budgets and a fresh client/server pair can be allocated at manager launch;
/// physical VM creation authority and an existing broker peer cannot be invented.
pub(super) fn validate_vm_sources(
    slots: &[Slot],
    plan: &crate::manifest::LaunchPlan<'_>,
    core_count: usize,
) -> hyper_os::Result<()> {
    use crate::bootstrap_policy::BootstrapAuthority;
    let has = |key| slots.iter().any(|slot| slot.key == key);
    for service in core_count..plan.service_count() {
        for capability in 0..crate::manifest::MAX_CAPABILITIES_PER_SERVICE {
            let Some(grant) = plan.capability_grant(service, capability) else {
                continue;
            };
            let unavailable = match BootstrapAuthority::from_key(grant.authority()) {
                Some(BootstrapAuthority::VmAuthority) => !has(VM_AUTHORITY),
                Some(BootstrapAuthority::IoBrokerClient) => !has(IO_BROKER_CLIENT),
                Some(BootstrapAuthority::VmManagerConnectionChannel) => {
                    has(VM_CLIENT) && !has(VM_MANAGER)
                }
                _ => false,
            };
            if unavailable {
                return Err(hyper_os::Error::MissingHandle);
            }
        }
    }
    Ok(())
}

pub(super) fn prepare_receive(
    slots: &[Slot],
    core_count: usize,
) -> hyper_os::Result<Vec<CapabilityReceiveSlot>> {
    validate_inventory(slots, core_count)?;
    slots.iter().map(receive_slot).collect()
}

/// Refuse a root-switch plan before accepting supervision if the sender cannot
/// delegate the namespace management authority required to carry it out.
pub(super) fn validate_namespace_source(slots: &[Slot]) -> hyper_os::Result<()> {
    let root = slots
        .iter()
        .find(|slot| slot.key == ROOT_DIRECTORY)
        .ok_or(hyper_os::Error::MissingHandle)?;
    let rights = Rights::from_bits(root.rights).ok_or(hyper_os::Error::InvalidResponse)?;
    if !rights.contains(
        Rights::MANAGE_NAMESPACE
            .union(Rights::READ)
            .union(Rights::WRITE)
            .union(Rights::EXECUTE),
    ) {
        return Err(hyper_os::Error::MissingHandle);
    }
    Ok(())
}

struct Received<'a> {
    descriptors: &'a [Slot],
    slots: &'a mut [CapabilityReceiveSlot],
}
impl Received<'_> {
    fn optional<T: TypedObject>(&mut self, key: u16) -> hyper_os::Result<Option<OwnedHandle<T>>> {
        match self.descriptors.iter().position(|s| s.key == key) {
            Some(index) => self.slots[index].take::<T>(),
            None => Ok(None),
        }
    }
    fn required<T: TypedObject>(&mut self, key: u16) -> hyper_os::Result<OwnedHandle<T>> {
        self.optional(key)?.ok_or(hyper_os::Error::MissingHandle)
    }
}

pub(super) fn adopt(
    descriptors: &[Slot],
    slots: &mut [CapabilityReceiveSlot],
) -> hyper_os::Result<Runtime> {
    let mut r = Received { descriptors, slots };
    let vm_fleet = if descriptors.iter().any(|s| s.key == VM_GROUP) {
        Some(VmFleet {
            group: r.required(VM_GROUP)?,
            domain: r.required(VM_DOMAIN)?,
            provisioning: r.optional(VM_PROVISIONING)?,
            provisioner: r.optional(PROVISIONER)?.map(|handle| FleetProvisioner {
                channel: CapabilityChannel::from_handle(handle),
            }),
        })
    } else {
        None
    };
    let vm_connection = if descriptors.iter().any(|s| s.key == VM_CLIENT) {
        Some(VmManagerConnection {
            client: r.required(VM_CLIENT)?,
            server: r.optional(VM_MANAGER)?,
        })
    } else {
        None
    };
    let authorities = AuthorityInventory {
        root_directory: Directory::from_handle(r.required(ROOT_DIRECTORY)?),
        boot_environment: Directory::from_handle(r.required(BOOT_ENVIRONMENT)?),
        system_environment: Some(Directory::from_handle(r.required(SYSTEM_ENVIRONMENT)?)),
        library_directory: Directory::from_handle(r.required(LIBRARY_DIRECTORY)?),
        factory: r.required(FACTORY)?,
        group: r.required(GROUP)?,
        domain: r.required(DOMAIN)?,
        task_inspector: r.required(TASK_INSPECTOR)?,
        object_inspector: r.required(OBJECT_INSPECTOR)?,
        memory_inspector: r.required(MEMORY_INSPECTOR)?,
        cpu_inspector: r.required(CPU_INSPECTOR)?,
        service_output_channel: r.required(SERVICE_OUTPUT)?,
        device: r.optional(DEVICE)?,
        console_input_channel: r.optional(CONSOLE_INPUT)?,
        console_output_channel: r.optional(CONSOLE_OUTPUT)?,
        session_input_channel: r.optional(SESSION_INPUT)?,
        session_output_channel: r.optional(SESSION_OUTPUT)?,
        session_startup_channel: r.optional(SESSION_STARTUP)?,
        session_startup_writer: r.optional(SESSION_DECISION)?,
        io_broker_server: r.optional(IO_BROKER_SERVER)?,
        io_broker_client: r.optional(IO_BROKER_CLIENT)?,
        io_ready_channel: r.optional(IO_READY)?,
        filesystem_attach: r.optional(FILESYSTEM_ATTACH)?,
        filesystem_provider: r.optional(FILESYSTEM_PROVIDER)?,
        filesystem_control_server: r.optional(FILESYSTEM_CONTROL_SERVER)?,
        filesystem_control_client: r.optional(FILESYSTEM_CONTROL_CLIENT)?,
        vm_creation: r.optional(VM_AUTHORITY)?,
        vm_fleet,
        vm_connection,
        console: r.required(CONSOLE)?,
    };
    let mut processes = std::array::from_fn(|_| None);
    for (index, process) in processes.iter_mut().enumerate() {
        *process = r.optional(PROCESS_BASE + index as u16)?;
    }
    Ok(Runtime {
        launcher: ServiceLauncher { authorities },
        supervisors: SupervisorSet { processes },
        _stage_group: Some(r.required(STAGE_GROUP)?),
        system_init: None,
        core_manifest: None,
        adopted: true,
        boot_failed: false,
    })
}

#[cfg(test)]
#[path = "../../../tests/handoff_inventory.rs"]
mod tests;
