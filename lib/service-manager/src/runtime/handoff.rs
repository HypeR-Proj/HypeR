// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! A capability rendezvous is the only supervisor-ownership commit point.
//! The candidate parses its complete service graph before registering receive.
//! Failure before commit leaves every Process owner in the boot supervisor.

mod inventory;
mod metadata;

use std::io::Read;
use std::mem::MaybeUninit;
use std::time::Duration;

use hyper_os::capability_channel::{CapabilityChannel, CapabilityDisposition};
use hyper_os::handle::{
    CapabilityChannelObject, OwnedHandle, ProcessObject, Rights, RightsOffer, TaskGroupObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{ProcessBuilder, create_task_group};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use serde::{Deserialize, Serialize};

use super::{Error, LaunchError, Runtime};
use crate::bootstrap_policy::BootstrapPolicy;
use crate::manifest::{self, MAX_SERVICES, Manifest};
use inventory::InventoryOffer;
use metadata::PreparedOffer;

const COMMIT: &[u8] = b"HYPER-INIT-HANDOFF/1";
const HANDOFF_TIMEOUT: Duration = Duration::from_secs(30);
const ENDPOINT: startup::StartupPurpose<CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8008_0001);
const _: () = assert!(MAX_SERVICES + 2 <= hyper_os::wait::MAX_ITEMS);

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SystemInitConfig {
    #[serde(rename = "system-init")]
    image: String,
    services: String,
    #[serde(default, rename = "root-switch")]
    root_switch: Option<super::root_switch::Config>,
}

impl SystemInitConfig {
    pub(super) fn load() -> Result<Option<Self>, Error> {
        let file = match std::fs::File::open("/etc/hyper/init.json") {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Source),
        };
        let mut text = String::new();
        file.take(4097)
            .read_to_string(&mut text)
            .map_err(|_| Error::Source)?;
        if text.len() > 4096 {
            return Err(Error::Source);
        }
        let next: Self = serde_json::from_str(&text).map_err(|_| Error::Source)?;
        next.validate()?;
        Ok(Some(next))
    }

    pub(super) fn validate(&self) -> Result<(), Error> {
        if !self.image.starts_with('/') || !self.services.starts_with('/') {
            return Err(Error::Source);
        }
        if let Some(config) = &self.root_switch {
            config.validate().map_err(|_| Error::Source)?;
        }
        Ok(())
    }
}

/// Starts a candidate with no core supervisor or management authority. Only
/// the final rendezvous grants its validated inventory. Duplicate group leases
/// overlap until boot exits, so no zero-active-handles stop can race the move.
pub(super) fn transfer(
    runtime: &mut Runtime,
    manifest: &Manifest<'_>,
    next: &SystemInitConfig,
    core: &str,
) -> Result<bool, LaunchError> {
    match try_transfer(runtime, manifest, next, core) {
        Ok(committed) => Ok(committed),
        Err(error) => {
            let _ = runtime
                .launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(
                    format!("HypeR init: system init handoff rejected: {error:?}\n").as_bytes(),
                );
            Ok(false)
        }
    }
}

fn try_transfer(
    runtime: &mut Runtime,
    manifest: &Manifest<'_>,
    next: &SystemInitConfig,
    core: &str,
) -> hyper_os::Result<bool> {
    let a = &runtime.launcher.authorities;
    let candidate_group = create_task_group(a.factory.as_handle_ref(), a.domain.as_handle_ref())?;
    let offer = PreparedOffer::new(runtime, &candidate_group, next, core)?;
    let snapshot = offer.snapshot()?;
    let (sender, receiver) = CapabilityChannel::create()?;
    let executable = a
        .root_directory
        .open(&next.image, hyper_service::process::EXECUTABLE_FILE_RIGHTS)?;
    let loader = hyper_service::process::open_loader(
        a.system_environment
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?,
        hyper_service::process::LoaderKind::Dynamic,
    )?;
    let builder = ProcessBuilder::create(
        a.factory.as_handle_ref(),
        candidate_group.as_handle_ref(),
        a.domain.as_handle_ref(),
        executable.as_handle_ref(),
        loader.as_handle_ref(),
    )?;
    builder.set_name("init-system")?;
    builder.add_argument(&next.image)?;
    builder.add_handle_duplicate(
        a.root_directory.as_handle_ref(),
        startup::ROOT_DIRECTORY.as_raw(),
        // libhyper retains a root by duplicating it, and std file opens request
        // INSPECT on the derived file. Preparation needs no filesystem writes.
        RightsOffer::Exact(
            Rights::READ
                .union(Rights::EXECUTE)
                .union(Rights::INSPECT)
                .union(Rights::DUPLICATE),
        ),
    )?;
    builder.add_handle_duplicate(
        a.library_directory.as_handle_ref(),
        startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
        RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE).union(Rights::PROTECT)),
    )?;
    builder.add_handle_duplicate(
        a.console.as_handle_ref(),
        startup::CONSOLE.as_raw(),
        RightsOffer::Exact(
            Rights::WRITE
                .union(Rights::WAIT)
                .union(Rights::DUPLICATE)
                .union(Rights::TRANSFER),
        ),
    )?;
    builder.add_handle_duplicate(
        snapshot.as_handle_ref(),
        metadata::SNAPSHOT.as_raw(),
        RightsOffer::Exact(Rights::READ.union(Rights::INSPECT)),
    )?;
    builder
        .add_handle_move(
            receiver.into_handle(),
            ENDPOINT.as_raw(),
            RightsOffer::Exact(Rights::WAIT.union(Rights::READ)),
        )
        .map_err(|failure| failure.into_parts().0)?;
    builder.seal()?;
    let process = builder.start().map_err(|failure| match failure {
        hyper_os::task::StartFailure::Rejected { error, .. }
        | hyper_os::task::StartFailure::Committed(error) => error,
    })?;
    let deadline = hyper_os::time::deadline_after(HANDOFF_TIMEOUT)?.as_raw();
    let result = commit_when_ready(
        runtime,
        manifest,
        &candidate_group,
        &process,
        &sender,
        &offer,
        deadline,
    );
    match result {
        Ok(true) => {
            // Process owners have moved. No further fallible operation here can
            // turn success into a rollback; an ACK is deliberately unnecessary.
            let _ = runtime
                .launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(b"HypeR init: core supervision transferred; boot init exiting\n");
            Ok(true)
        }
        other => {
            // The staging group contains only init-system. Never stop the core
            // group on failed preparation, timeout, or peer loss.
            let _ = process.as_process_supervisor().request_stop();
            let join_deadline = hyper_os::time::deadline_after(Duration::from_secs(5))?.as_raw();
            process
                .as_process_supervisor()
                .wait_terminated(join_deadline)?;
            other
        }
    }
}

fn commit_when_ready(
    runtime: &mut Runtime,
    manifest: &Manifest<'_>,
    group: &OwnedHandle<TaskGroupObject>,
    candidate: &OwnedHandle<ProcessObject>,
    channel: &CapabilityChannel,
    offer: &PreparedOffer,
    deadline: u64,
) -> hyper_os::Result<bool> {
    // Retain terminal optional Process owners until commit/abort: removing one
    // would change the already advertised receive-slot contract.
    let mut observed = [false; MAX_SERVICES];
    loop {
        let mut waits = Vec::with_capacity(MAX_SERVICES + 2);
        let mut indexes = Vec::with_capacity(MAX_SERVICES);
        for (index, process) in runtime.supervisors.processes.iter().enumerate() {
            if let Some(process) = process
                && !observed[index]
            {
                indexes.push(index);
                waits.push(WaitItem::new(
                    process.as_handle_ref(),
                    ObjectSignals::<ProcessObject>::TERMINATED,
                ));
            }
        }
        let candidate_index = waits.len();
        waits.push(WaitItem::new(
            candidate.as_handle_ref(),
            ObjectSignals::<ProcessObject>::TERMINATED,
        ));
        let channel_index = waits.len();
        waits.push(WaitItem::new(
            channel.as_handle_ref(),
            ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
        ));
        let observation = wait_many(&waits, deadline)?;
        if observation.index < candidate_index {
            let index = indexes[observation.index];
            observed[index] = true;
            if manifest
                .service(index)
                .is_none_or(|service| service.critical())
            {
                return Ok(false);
            }
            continue;
        }
        if observation.index == candidate_index {
            return Ok(false);
        }
        if observation.index != channel_index
            || !ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                .is_present_in(observation.observed)
        {
            return Ok(false);
        }
        drop(waits);
        let mut inventory = InventoryOffer::new(&runtime.launcher.authorities, group)?;
        inventory.describe_processes(&runtime.supervisors)?;
        if inventory.slots != offer.slots {
            return Err(hyper_os::Error::InvalidResponse);
        }
        // Borrow supervisors separately from the immutable management inventory.
        for process in runtime
            .supervisors
            .processes
            .iter_mut()
            .filter(|p| p.is_some())
        {
            inventory
                .dispositions
                .push(CapabilityDisposition::move_handle(
                    process,
                    RightsOffer::SameRights,
                )?);
        }
        match channel.try_send(COMMIT, &mut inventory.dispositions) {
            Ok(()) => return Ok(true),
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
            Err(error) => return Err(error),
        }
    }
}

pub(super) fn run_system(startup: &mut Startup<'_>) -> Result<(), Error> {
    hyper_os::require_core_abi().map_err(|error| {
        report_failure("checking the Native ABI", &error);
        Error::OperatingSystem
    })?;
    let offer = PreparedOffer::read(startup).map_err(|error| {
        report_failure("reading the handoff offer", &error);
        Error::Source
    })?;
    let text = metadata::read_manifest(&offer.next.services)?;
    let core = manifest::parse(&offer.core_manifest).map_err(|_| Error::Source)?;
    let core_count = core.service_count();
    let system = manifest::parse(&text).map_err(|_| Error::Source)?;
    let manifest = core.append_system(system).ok_or(Error::Source)?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|_| Error::Source)?;
    Runtime::preflight(&manifest).map_err(|_| Error::Source)?;
    inventory::validate_vm_sources(&offer.slots, &plan, core_count).map_err(|_| Error::Source)?;
    inventory::validate_session_source(&offer.slots, &plan, core_count)
        .map_err(|_| Error::Source)?;
    if offer.next.root_switch.is_some() {
        inventory::validate_namespace_source(&offer.slots).map_err(|_| Error::Source)?;
    }
    let mut slots =
        inventory::prepare_receive(&offer.slots, core_count).map_err(|_| Error::Source)?;
    let endpoint = CapabilityChannel::from_handle(startup.take(ENDPOINT).map_err(|error| {
        report_failure("obtaining the handoff endpoint", &error);
        Error::OperatingSystem
    })?);
    let mut bytes = [MaybeUninit::uninit(); COMMIT.len()];
    // Registering this exact receive is READY. Until the syscall returns OK,
    // this process owns no core supervisors and cannot interfere with recovery.
    let message = endpoint
        .receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)
        .map_err(|error| {
            report_failure("receiving core supervision", &error);
            Error::OperatingSystem
        })?;
    if message.bytes() != COMMIT || message.capability_count() != slots.len() {
        return Err(Error::Source);
    }
    let mut runtime = inventory::adopt(&offer.slots, &mut slots).map_err(|error| {
        report_failure("adopting core supervision", &error);
        Error::OperatingSystem
    })?;
    runtime.boot_failed = offer.boot_failed;
    let _ = runtime
        .launcher
        .authorities
        .console
        .as_emergency_console()
        .write_all(b"HypeR init-system: core supervision adopted\n");
    if let Some(config) = &offer.next.root_switch {
        if let Err(error) = config.apply(&runtime.launcher.authorities.root_directory) {
            let _ = runtime.launcher.authorities.console.as_emergency_console().write_all(
                format!("HypeR init-system: root switch failed: {error}; retaining core supervision and recovery shell\n").as_bytes(),
            );
            // The rendezvous already moved every Process owner here. Failure
            // to switch roots is recoverable; dropping this runtime would stop
            // the very services needed to repair the deployment.
            runtime
                .activate_console(hyper_service::session::ShellMode::Recovery)
                .map_err(|_| Error::Bootstrap)?;
            return runtime
                .supervisors
                .supervise(&manifest, &runtime.launcher.authorities.console)
                .map(|never| match never {})
                .map_err(|_| Error::Bootstrap);
        }
        let _ = runtime
            .launcher
            .authorities
            .console
            .as_emergency_console()
            .write_all(
                format!(
                    "HypeR init-system: root namespace switched; bootstrap retained at {}\n",
                    config.put_old()
                )
                .as_bytes(),
            );
    }
    runtime
        .launch_validated_graph(&manifest, &plan)
        .map_err(|error| {
            let _ = runtime
                .launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(error.diagnostic());
            Error::Bootstrap
        })
}

fn report_failure(context: &str, error: &impl std::fmt::Debug) {
    if let Ok(console) = hyper_rt::process::console() {
        let _ = console
            .as_emergency_console()
            .write_all(format!("HypeR init-system: failed {context}: {error:?}\n").as_bytes());
    }
}
