// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

mod authority;
mod handoff;
mod launcher;
mod recovery;
mod report;
mod supervisor;
mod vm_connection;

use crate::bootstrap_policy::BootstrapPolicy;
use authority::AuthorityInventory;
use launcher::ServiceLauncher;
use supervisor::{StorageReadiness, SupervisorSet};

use std::io::Read;

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
    let source = load_manifest()?;
    let manifest = crate::manifest::parse(&source).map_err(|_| Error::Source)?;
    let plan = crate::manifest::validate(&manifest, &BootstrapPolicy).map_err(|_| Error::Source)?;
    let mut runtime = Runtime::from_startup(startup, root_directory)?;
    let system_init = match handoff::SystemInitConfig::load() {
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
            None
        }
    };
    runtime
        .launch_validated_graph(&manifest, &plan, &source, system_init.as_ref())
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

fn load_manifest() -> Result<String, Error> {
    let file = std::fs::File::open(MANIFEST_PATH).map_err(|_| Error::OperatingSystem)?;
    let mut text = String::new();
    file.take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|_| Error::Source)?;
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(Error::Source);
    }
    Ok(text)
}

/// Top-level init state. Subobjects own disjoint authority and lifecycle roles.
struct Runtime {
    launcher: ServiceLauncher,
    supervisors: SupervisorSet,
    storage_ready: bool,
}

impl Runtime {
    fn from_startup(startup: &mut Startup<'_>, root_directory: Directory) -> Result<Self, Error> {
        let (raw_control_server, raw_controller) =
            CapabilityChannel::create().map_err(|_| Error::OperatingSystem)?;
        let (log_server, log_connector) =
            CapabilityChannel::create().map_err(|_| Error::OperatingSystem)?;
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
            raw_control_server: Some(raw_control_server.into_handle()),
            raw_controller: Some(raw_controller.into_handle()),
            log_server: Some(log_server.into_handle()),
            log_connector: log_connector.into_handle(),
            service_output_channel: None,
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
            storage_ready: true,
            supervisors: SupervisorSet {
                processes: std::array::from_fn(|_| None),
            },
        })
    }

    fn preflight(manifest: &Manifest<'_>) -> Result<(), LaunchError> {
        if manifest.vm_config_path().is_some()
            || manifest
                .services()
                .any(|service| service.phase() != ServicePhase::Boot)
        {
            return Err(LaunchError::UnsupportedSupervisionGraph);
        }
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
    /// Starts core services and offers supervision to the successor.
    ///
    /// A failed mount keeps the physical backend and filesystem manager alive
    /// for recovery. Only committed handoff ends boot supervision.
    /// Failures after service launch request service shutdown before return.
    #[inline(never)]
    fn launch_validated_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
        core_manifest: &str,
        system_init: Option<&handoff::SystemInitConfig>,
    ) -> Result<(), LaunchError> {
        Self::preflight(manifest)?;
        {
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
        let ready_reader = if ready_service.is_some() {
            let (writer, reader) =
                channel::create_pair().map_err(|_| LaunchError::OperatingSystem)?;
            self.launcher.authorities.io_ready_channel = Some(writer);
            Some(reader)
        } else {
            None
        };
        if plan
            .unique_service_for_purpose(vm_contract::PROVISIONING.as_raw())
            .is_some()
        {
            return Err(LaunchError::UnsupportedSupervisionGraph);
        }
        let result = (|| {
            {
                self.launcher
                    .start_services(manifest, plan, &mut self.supervisors)?;
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
                }
            }
            drop(ready_reader);
            self.storage_ready = storage_available;
            if let Some(next) = system_init {
                // Only bootstrap mounts precede handoff. System-init availability
                // is decided by its executable and library capabilities.
                if self.prepare_system_libraries()?
                    && handoff::transfer(self, manifest, next, core_manifest)
                {
                    // The successful rendezvous moved all process owners.
                    // No later acknowledgement can reopen rollback.
                    return Ok(());
                }
            }
            self.launcher.authorities.close_unstarted_vm_manager();
            let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                b"HypeR init: system init unavailable; retaining core supervision and recovery shell\n");
            self.recovery_console(manifest)
        })();
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = self
                    .launcher
                    .authorities
                    .console
                    .as_emergency_console()
                    .write_all(error.diagnostic());
                // Recovery owns input only before a committed handoff. Do not
                // tear down healthy storage merely because another service failed.
                self.recovery_console(manifest)
            }
        }
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
    UnsupportedAuthority,
    UnsupportedRestartPolicy,
    UnsupportedSupervisionGraph,
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
            Self::UnsupportedAuthority => b"HypeR init: unsupported service authority\n",
            Self::UnsupportedRestartPolicy => b"HypeR init: unsupported restart policy\n",
            Self::UnsupportedSupervisionGraph => {
                b"HypeR init: unsupported critical-service supervision graph\n"
            }
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
            Self::UnsupportedAuthority => b"unsupported authority",
            Self::UnsupportedRestartPolicy => b"unsupported restart policy",
            Self::UnsupportedSupervisionGraph => b"unsupported supervision graph",
            Self::WaitTimedOut => b"service handshake timed out",
        }
    }
}
