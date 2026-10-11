// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

mod authority;
mod fleet;
mod handoff;
mod launcher;
mod provision;
mod report;
mod root_switch;
mod supervisor;

use crate::bootstrap_policy::BootstrapPolicy;
use authority::AuthorityInventory;
use launcher::ServiceLauncher;
use provision::FleetConfiguration;
use supervisor::{StorageReadiness, SupervisorSet};

use std::io::Read;

use crate::ManifestSource;
use crate::manifest::{LaunchPlan, MAX_MANIFEST_BYTES, Manifest, ServicePhase};
use crate::supervision::{self, SupportError};
use hyper_os::capability_channel::CapabilityChannel;
use hyper_os::channel;
use hyper_os::fs::Directory;
use hyper_os::startup::{self, Startup};
use hyper_service::vm as vm_contract;

const MANIFEST_PATH: &str = "/etc/hyper/services.json";

#[inline(never)]
pub fn run(startup: &mut Startup<'_>) -> Result<(), Error> {
    hyper_os::require_core_abi().map_err(|_| Error::OperatingSystem)?;
    let root_directory = startup
        .take_root_directory()
        .map_err(|_| Error::OperatingSystem)?;
    let source = LoadedManifest::load()?;
    let manifest = crate::manifest::parse(&source.0).map_err(|_| Error::Source)?;
    let plan = crate::manifest::validate(&manifest, &BootstrapPolicy).map_err(|_| Error::Source)?;
    let mut runtime = Runtime::from_startup(startup, root_directory)?;
    runtime.system_init = match handoff::SystemInitConfig::load() {
        Ok(next) => next,
        Err(_) => {
            let _ = runtime
                .launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(
                    b"HypeR init: invalid system-init configuration; retaining recovery services\n",
                );
            runtime.boot_failed = true;
            None
        }
    };
    runtime.core_manifest = Some(source.0.clone());
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

/// Receives the boot supervisor's committed inventory before launching services.
pub fn run_system(startup: &mut Startup<'_>) -> Result<(), Error> {
    handoff::run_system(startup)
}

pub enum Error {
    Bootstrap,
    OperatingSystem,
    Source,
}

impl Error {
    pub const fn diagnostic(&self) -> &'static [u8] {
        match self {
            Self::Bootstrap => b"HypeR init: bootstrap failed\n",
            Self::OperatingSystem => b"HypeR init: startup capability operation failed\n",
            Self::Source => b"HypeR init: invalid service manifest source\n",
        }
    }
}

struct LoadedManifest(String);

impl LoadedManifest {
    fn load() -> Result<Self, Error> {
        let file = std::fs::File::open(MANIFEST_PATH).map_err(|_| Error::OperatingSystem)?;
        let mut bytes = Vec::new();
        file.take(MAX_MANIFEST_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::OperatingSystem)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(Error::Source);
        }
        String::from_utf8(bytes)
            .map(Self)
            .map_err(|_| Error::Source)
    }
}

type SourceError = std::convert::Infallible;

impl ManifestSource for LoadedManifest {
    type Error = SourceError;
    fn manifest(&self) -> Result<&str, Self::Error> {
        Ok(&self.0)
    }
}

/// Top-level init state. Subobjects own disjoint authority and lifecycle roles.
struct Runtime {
    launcher: ServiceLauncher,
    supervisors: SupervisorSet,
    system_init: Option<handoff::SystemInitConfig>,
    core_manifest: Option<String>,
    adopted: bool,
    boot_failed: bool,
    _stage_group: Option<hyper_os::OwnedHandle<hyper_os::handle::TaskGroupObject>>,
}

impl Runtime {
    fn from_startup(startup: &mut Startup<'_>, root_directory: Directory) -> Result<Self, Error> {
        let (console_input_channel, session_input_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (session_output_channel, console_output_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let (session_startup_writer, session_startup_channel) =
            channel::create_pair().map_err(|_| Error::OperatingSystem)?;
        let factory = startup
            .take(startup::TASK_FACTORY)
            .map_err(|_| Error::OperatingSystem)?;
        let group = startup
            .take(startup::TASK_GROUP)
            .map_err(|_| Error::OperatingSystem)?;
        let domain = startup
            .take(startup::RESOURCE_DOMAIN)
            .map_err(|_| Error::OperatingSystem)?;
        let boot_environment = hyper_service::process::open_environment(&root_directory, "/")
            .map_err(|_| Error::OperatingSystem)?;
        let library_directory = hyper_service::process::open_library_directory(&boot_environment)
            .map_err(|_| Error::OperatingSystem)?;
        let authorities = AuthorityInventory {
            root_directory,
            boot_environment,
            system_environment: None,
            library_directory,
            factory,
            group,
            domain,
            vm_creation: startup
                .take_optional(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)
                .map_err(|_| Error::OperatingSystem)?,
            vm_fleet: None,
            vm_connection: None,
            device: startup
                .take_optional(startup::DEVICE_ASSIGNMENT_AUTHORITY)
                .map_err(|_| Error::OperatingSystem)?,
            task_inspector: startup
                .take(startup::TASK_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            object_inspector: startup
                .take(startup::OBJECT_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            memory_inspector: startup
                .take(startup::MEMORY_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            cpu_inspector: startup
                .take(startup::CPU_INSPECTOR)
                .map_err(|_| Error::OperatingSystem)?,
            console: {
                let console = hyper_rt::process::console().map_err(|_| Error::OperatingSystem)?;
                console
                    .duplicate(console.info().map_err(|_| Error::OperatingSystem)?.rights)
                    .map_err(|_| Error::OperatingSystem)?
            },
            console_input_channel: Some(console_input_channel),
            console_output_channel: Some(console_output_channel),
            session_input_channel: Some(session_input_channel),
            // Share only the writer with services. Output bypasses session so
            // session's own logging never depends on its relay loop draining it.
            service_output_channel: session_output_channel
                .duplicate(hyper_service::stdio::STANDARD_OUTPUT_CONTRACT.allowed_rights())
                .map_err(|_| Error::OperatingSystem)?,
            session_output_channel: Some(session_output_channel),
            session_startup_channel: Some(session_startup_channel),
            session_startup_writer: Some(session_startup_writer),
            io_ready_channel: None,
            io_broker_server: None,
            io_broker_client: None,
            filesystem_attach: None,
            filesystem_provider: None,
            filesystem_control_server: None,
            filesystem_control_client: None,
        };
        Ok(Self {
            launcher: ServiceLauncher { authorities },
            system_init: None,
            core_manifest: None,
            adopted: false,
            boot_failed: false,
            _stage_group: None,
            supervisors: SupervisorSet {
                processes: std::array::from_fn(|_| None),
            },
        })
    }

    fn preflight(manifest: &Manifest<'_>) -> Result<(), LaunchError> {
        match supervision::validate(manifest) {
            Ok(()) => Ok(()),
            Err(SupportError::RestartPolicy) => Err(LaunchError::UnsupportedRestartPolicy),
            Err(SupportError::MissingCriticalService) => {
                Err(LaunchError::UnsupportedSupervisionGraph)
            }
        }
    }
}

impl Runtime {
    /// Starts the service graph, provisions the VM fleet, then supervises services.
    ///
    /// A failed mount keeps the physical backend and filesystem manager alive
    /// for recovery. Only committed handoff ends boot supervision.
    /// Failures after service launch request service shutdown before return.
    #[inline(never)]
    fn launch_validated_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
    ) -> Result<(), LaunchError> {
        Self::preflight(manifest)?;
        if !self.adopted {
            if crate::bootstrap_policy::filesystem_enabled(plan)
                .map_err(|_| LaunchError::InvalidPlan)?
            {
                let (manager, provider) =
                    CapabilityChannel::create().map_err(|_| LaunchError::OperatingSystem)?;
                self.launcher.authorities.filesystem_attach = Some(manager.into_handle());
                self.launcher.authorities.filesystem_provider = Some(provider.into_handle());
            }
            if plan
                .unique_service_for_purpose(hyper_service::io::BROKER_SERVER.as_raw())
                .is_some()
            {
                let (server, client) =
                    CapabilityChannel::create().map_err(|_| LaunchError::OperatingSystem)?;
                self.launcher.authorities.io_broker_server = Some(server.into_handle());
                self.launcher.authorities.io_broker_client = Some(client.into_handle());
            }
            if plan
                .unique_service_for_purpose(hyper_service::filesystem::CONTROL_SERVER.as_raw())
                .is_some()
            {
                let (server, client) =
                    CapabilityChannel::create().map_err(|_| LaunchError::OperatingSystem)?;
                self.launcher.authorities.filesystem_control_server = Some(server.into_handle());
                self.launcher.authorities.filesystem_control_client = Some(client.into_handle());
            }
        }
        let ready_service = crate::bootstrap_policy::io_ready_service(plan)
            .map_err(|_| LaunchError::InvalidPlan)?;
        if ready_service
            .and_then(|index| manifest.service(index))
            .is_some_and(|service| service.phase() != ServicePhase::Boot)
        {
            return Err(LaunchError::UnsupportedSupervisionGraph);
        }
        let ready_reader = if ready_service.is_some() && !self.adopted {
            let (writer, reader) =
                channel::create_pair().map_err(|_| LaunchError::OperatingSystem)?;
            self.launcher.authorities.io_ready_channel = Some(writer);
            Some(reader)
        } else {
            None
        };
        let vm_manager_index = plan.unique_service_for_purpose(vm_contract::PROVISIONING.as_raw());
        if plan.vm_config_path().is_some() && vm_manager_index.is_none() {
            return Err(LaunchError::InvalidPlan);
        }
        let result = (|| {
            if !self.adopted {
                self.launcher.start_phase(
                    manifest,
                    plan,
                    vm_manager_index,
                    &mut self.supervisors,
                    ServicePhase::Boot,
                )?;
            }
            let mut storage_available = true;
            if let (Some(reader), Some(service)) = (ready_reader.as_ref(), ready_service) {
                let readiness = self.supervisors.wait_for_storage(
                    manifest,
                    service,
                    reader,
                    &self.launcher.authorities.console,
                )?;
                if let StorageReadiness::Unavailable(reason) = readiness {
                    let _ = self
                        .launcher
                        .authorities
                        .console
                        .as_emergency_console()
                        .write_all(reason.diagnostic());
                    storage_available = false;
                    self.boot_failed = true;
                }
            }
            drop(ready_reader);
            if let Some(next) = self.system_init.take() {
                // /system may already be in the ramfs, or mounted successfully
                // while an unrelated data volume failed. System-init availability
                // is decided by its executable and library capabilities.
                if self.prepare_system_libraries()? {
                    let text = self.core_manifest.take().ok_or(LaunchError::InvalidPlan)?;
                    if handoff::transfer(self, manifest, &next, &text)? {
                        // The successful rendezvous moved all process owners.
                        // No later acknowledgement can reopen rollback.
                        return Ok(());
                    }
                }
                self.configure_vm_fleet(manifest, plan, vm_manager_index, storage_available)?;
                self.activate_console(hyper_service::session::ShellMode::Recovery)?;
                let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                    b"HypeR init: system init unavailable; retaining core supervision and recovery shell\n");
                return self
                    .supervisors
                    .supervise(manifest, &self.launcher.authorities.console)
                    .map(|never| match never {});
            }
            if manifest
                .services()
                .any(|service| service.phase() == ServicePhase::System)
            {
                if storage_available {
                    storage_available = self.prepare_system_libraries()?;
                }
                if storage_available {
                    self.launcher.start_phase(
                        manifest,
                        plan,
                        vm_manager_index,
                        &mut self.supervisors,
                        ServicePhase::System,
                    )?;
                } else {
                    let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                        b"HypeR init: system services unavailable; recovery shell remains available\n",
                    );
                }
            }
            self.configure_vm_fleet(manifest, plan, vm_manager_index, storage_available)?;
            self.activate_console(if self.boot_failed || !storage_available {
                hyper_service::session::ShellMode::Recovery
            } else {
                hyper_service::session::ShellMode::System
            })?;
            self.supervisors
                .supervise(manifest, &self.launcher.authorities.console)
                .map(|never| match never {})
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                let session = plan
                    .unique_service_for_purpose(hyper_service::session::STARTUP_DECISION.as_raw());
                if self.launcher.authorities.session_startup_writer.is_some()
                    && session.is_some_and(|index| {
                        self.supervisors.processes[index]
                            .as_ref()
                            .is_some_and(|owner| {
                                owner
                                    .as_process_supervisor()
                                    .info()
                                    .is_ok_and(|info| info.terminal.is_none())
                            })
                    })
                {
                    let _ = self
                        .launcher
                        .authorities
                        .console
                        .as_emergency_console()
                        .write_all(error.diagnostic());
                    self.activate_console(hyper_service::session::ShellMode::Recovery)?;
                    return self
                        .supervisors
                        .supervise(manifest, &self.launcher.authorities.console)
                        .map(|never| match never {});
                }
                if let Err(stop_error) = self.supervisors.request_service_stop() {
                    let _ = self
                        .launcher
                        .authorities
                        .console
                        .as_emergency_console()
                        .write_all(stop_error.diagnostic());
                }
                Err(error)
            }
        }
    }

    /// Consume the sole decision writer only after boot has a terminal outcome.
    /// The channel is part of handoff ownership; a successful transfer does not
    /// publish a decision from the departing supervisor.
    fn activate_console(
        &mut self,
        mode: hyper_service::session::ShellMode,
    ) -> Result<(), LaunchError> {
        if let Some(writer) = self.launcher.authorities.session_startup_writer.take() {
            match writer.as_byte_channel().try_send(&[mode.encode()]) {
                Ok(()) => {}
                // Session's bounded wait already chose recovery and closed
                // its endpoint. Do not tear down core services on late boot.
                Err(hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED)) => {}
                Err(_) => return Err(LaunchError::OperatingSystem),
            }
        }
        Ok(())
    }

    /// Finish only the business-service handshake. Storage ownership never
    /// depends on a manager being configured, started, or successfully admitted.
    fn configure_vm_fleet(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
        vm_manager_index: Option<usize>,
        storage_available: bool,
    ) -> Result<(), LaunchError> {
        if let Some(manager) = vm_manager_index {
            let configuration = if self.supervisors.processes[manager].is_none() {
                // A system-volume failure must not wait for an unstarted
                // manager. Close its retained endpoints so clients fail
                // promptly while bootstrap services remain supervised.
                self.launcher.authorities.close_unstarted_vm_manager();
                FleetConfiguration::Unavailable
            } else if let Some(path) = plan.vm_config_path().filter(|_| storage_available) {
                self.take_fleet_provisioner()?.configure_fleet(
                    manifest,
                    manager,
                    path,
                    &self.launcher.authorities.root_directory,
                    &self.launcher.authorities.console,
                    &mut self.supervisors,
                )?
            } else {
                self.take_fleet_provisioner()?.configuration_unavailable(
                    manifest,
                    manager,
                    &self.launcher.authorities.console,
                    &mut self.supervisors,
                )?
            };
            if configuration == FleetConfiguration::Unavailable {
                self.boot_failed = true;
                report::report_fleet_unavailable(&self.launcher.authorities.console);
            }
        } else {
            self.launcher.authorities.close_unstarted_vm_manager();
        }
        Ok(())
    }

    fn take_fleet_provisioner(&mut self) -> Result<provision::FleetProvisioner, LaunchError> {
        self.launcher
            .authorities
            .vm_fleet
            .as_mut()
            .and_then(|fleet| fleet.provisioner.take())
            .ok_or(LaunchError::InvalidPlan)
    }

    fn prepare_system_libraries(&mut self) -> Result<bool, LaunchError> {
        let prepared = (|| {
            let environment = hyper_service::process::open_environment(
                &self.launcher.authorities.root_directory,
                "/system",
            )?;
            let libraries = hyper_service::process::open_library_directory(&environment)?;
            Ok((environment, libraries))
        })();
        match prepared {
            Ok((environment, libraries)) => {
                self.launcher.authorities.system_environment = Some(environment);
                self.launcher.authorities.library_directory = libraries;
                Ok(true)
            }
            Err(error) if supervision::service_unavailable(&error) => {
                let _ = self
                    .launcher
                    .authorities
                    .console
                    .as_emergency_console()
                    .write_all(
                        format!("HypeR init: cannot open system libraries: {error:?}\n").as_bytes(),
                    );
                Ok(false)
            }
            Err(_) => Err(LaunchError::OperatingSystem),
        }
    }
}

#[derive(Debug)]
pub(super) enum LaunchError {
    AuthorityConsumed,
    CriticalServiceTerminated,
    InvalidPlan,
    OperatingSystem,
    RequiredServiceUnavailable,
    ServiceUnavailable,
    StopRollbackFailed,
    UnsupportedAuthority,
    UnsupportedRestartPolicy,
    UnsupportedSupervisionGraph,
    VmFleetReplyClosed,
    VmFleetReplyInvalid,
    VmProvisioningClosed,
    WaitTimedOut,
}

impl LaunchError {
    fn service_operation(error: hyper_os::Error) -> Self {
        if supervision::service_unavailable(&error) {
            Self::ServiceUnavailable
        } else {
            Self::OperatingSystem
        }
    }

    fn service_image(error: hyper_os::Error) -> Self {
        if supervision::service_image_unavailable(&error) {
            Self::ServiceUnavailable
        } else {
            Self::OperatingSystem
        }
    }

    fn service_resources(error: hyper_os::Error) -> Self {
        if supervision::service_resources_unavailable(&error) {
            Self::ServiceUnavailable
        } else {
            Self::OperatingSystem
        }
    }

    const fn diagnostic(&self) -> &'static [u8] {
        match self {
            Self::AuthorityConsumed => b"HypeR init: service authority was already consumed\n",
            Self::CriticalServiceTerminated => b"HypeR init: critical service terminated\n",
            Self::InvalidPlan => b"HypeR init: invalid launch plan\n",
            Self::OperatingSystem => b"HypeR init: service launch operation failed\n",
            Self::RequiredServiceUnavailable => b"HypeR init: startup provider unavailable\n",
            Self::ServiceUnavailable => b"HypeR init: service image or resources unavailable\n",
            Self::StopRollbackFailed => b"HypeR init: service rollback failed\n",
            Self::UnsupportedAuthority => b"HypeR init: unsupported service authority\n",
            Self::UnsupportedRestartPolicy => b"HypeR init: unsupported restart policy\n",
            Self::UnsupportedSupervisionGraph => {
                b"HypeR init: unsupported critical-service supervision graph\n"
            }
            Self::VmFleetReplyClosed => {
                b"HypeR init: VM fleet result channel closed without a reply\n"
            }
            Self::VmFleetReplyInvalid => b"HypeR init: VM fleet configuration reply is invalid\n",
            Self::VmProvisioningClosed => b"HypeR init: VM provisioning channel closed\n",
            Self::WaitTimedOut => b"HypeR init: service handshake timed out\n",
        }
    }

    const fn reason(&self) -> &'static [u8] {
        match self {
            Self::AuthorityConsumed => b"authority already consumed",
            Self::CriticalServiceTerminated => b"critical service terminated",
            Self::InvalidPlan => b"invalid launch plan",
            Self::OperatingSystem => b"kernel operation rejected",
            Self::RequiredServiceUnavailable => b"startup provider unavailable",
            Self::ServiceUnavailable => b"service image or resources unavailable",
            Self::StopRollbackFailed => b"rollback failed",
            Self::UnsupportedAuthority => b"unsupported authority",
            Self::UnsupportedRestartPolicy => b"unsupported restart policy",
            Self::UnsupportedSupervisionGraph => b"unsupported supervision graph",
            Self::VmFleetReplyClosed => b"VM fleet result channel closed without a reply",
            Self::VmFleetReplyInvalid => b"VM fleet configuration reply is invalid",
            Self::VmProvisioningClosed => b"VM provisioning channel closed",
            Self::WaitTimedOut => b"service handshake timed out",
        }
    }
}
