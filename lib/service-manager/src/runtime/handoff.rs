// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Receive core supervision after validating the formal environment's policy.
//! Registering receive declares readiness; only a successful capability
//! rendezvous commits ownership.

mod inventory;
mod metadata;

use hyper_os::capability_channel::CapabilityChannel;
use hyper_os::handle::CapabilityChannelObject;
use hyper_os::startup::{self, Startup};
use std::mem::MaybeUninit;

use super::{Error, Runtime};
use crate::bootstrap_policy::BootstrapPolicy;
use crate::manifest;
use metadata::PreparedOffer;

const COMMIT: &[u8] = b"HYPER-INIT-HANDOFF/1";
const ENDPOINT: startup::StartupPurpose<CapabilityChannelObject> =
    startup::StartupPurpose::new(0x8008_0001);

pub(super) fn run_system(startup: &mut Startup<'_>) -> Result<(), Error> {
    hyper_os::require_core_abi().map_err(|error| {
        report_failure("checking the Native ABI", &error);
        Error::OperatingSystem
    })?;
    let offer = PreparedOffer::read(startup).map_err(|error| {
        report_failure("reading the handoff offer", &error);
        Error::Source
    })?;
    let config = super::config::Config::load()?;
    let text = metadata::read_manifest(&config.services)?;
    let core = manifest::parse(&offer.core_manifest).map_err(|_| Error::Source)?;
    let core_count = core.service_count();
    let system = manifest::parse(&text).map_err(|_| Error::Source)?;
    let manifest = core.append_system(system).ok_or(Error::Source)?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|_| Error::Source)?;
    Runtime::preflight(&manifest).map_err(|_| Error::Source)?;
    inventory::validate_vm_sources(&offer.slots, &plan, core_count).map_err(|_| Error::Source)?;
    inventory::validate_session_source(&offer.slots, &plan, core_count)
        .map_err(|_| Error::Source)?;
    if config.root_switch.is_some() {
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
    runtime.bootstrap_storage_ready = offer.storage_ready;
    let _ = runtime
        .launcher
        .authorities
        .console
        .as_emergency_console()
        .write_all(b"HypeR init-system: core supervision adopted\n");
    if let Some(config) = &config.root_switch {
        if let Err(error) = config.apply(&runtime.launcher.authorities.root_directory) {
            let _ = runtime.launcher.authorities.console.as_emergency_console().write_all(
                format!("HypeR init-system: root switch failed: {error}; retaining core supervision and system console\n").as_bytes(),
            );
            // The rendezvous already moved every Process owner here. Failure
            // to switch roots is recoverable; dropping this runtime would stop
            // the very services needed to repair the deployment.
            // Formal services can still start from their retained environment.
            // They do not depend on the namespace switch succeeding.
            return runtime
                .launch_validated_graph(&manifest, &plan)
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
