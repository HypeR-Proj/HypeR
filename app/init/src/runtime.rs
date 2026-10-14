// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped adapter from a validated manifest to `ProcessBuilder`.

mod adoption;
mod authority;
mod config;
mod fleet;
mod handoff;
mod launcher;
mod mounts;
mod provision;
mod report;
mod supervisor;

use launcher::ServiceLauncher;
use provision::FleetConfiguration;
use supervisor::SupervisorSet;

use crate::manifest::{LaunchPlan, Manifest, ServicePhase};
use crate::supervision::{self, SupportError};
use hyper_os::startup::Startup;
use hyper_service::vm as vm_contract;

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

/// Top-level init state. Subobjects own disjoint authority and lifecycle roles.
struct Runtime {
    launcher: ServiceLauncher,
    supervisors: SupervisorSet,
    bootstrap_storage_ready: bool,
    providers: [Option<usize>; 3],
    core_count: usize,
    retired_processes: Vec<hyper_os::OwnedHandle<hyper_os::handle::ProcessObject>>,
    // Started replacements are not mandatory services until READY confirms
    // adoption. An unconfirmed transfer keeps both process owners for recovery.
    pending_provider: Option<(
        usize,
        hyper_os::OwnedHandle<hyper_os::handle::ProcessObject>,
    )>,
    unconfirmed_providers: Vec<hyper_os::OwnedHandle<hyper_os::handle::ProcessObject>>,
    retirement_path: Option<String>,
    retirement_ready: bool,
    old_console: Option<hyper_os::OwnedHandle<hyper_os::handle::ProcessObject>>,
    // Kept separate from the formal launcher group so stopping the predecessor
    // cannot terminate system init or any newly started formal services.
    _boot_group: hyper_os::OwnedHandle<hyper_os::handle::TaskGroupObject>,
}

impl Runtime {
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
    /// for system-side diagnosis. Bootstrap no longer owns failure handling.
    /// Failures keep healthy services supervised under formal init ownership.
    #[inline(never)]
    fn launch_validated_graph(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
    ) -> Result<(), LaunchError> {
        Self::preflight(manifest)?;
        let vm_manager_index = plan.unique_service_for_purpose(vm_contract::PROVISIONING.as_raw());
        if plan.vm_config_path().is_some() && vm_manager_index.is_none() {
            return Err(LaunchError::InvalidPlan);
        }
        let result = (|| {
            // The candidate needs only read access before the rendezvous.
            // Publish writable temporary storage after ownership commits, so
            // rejected candidates leave the old namespace wholly unchanged.
            hyper_os::filesystem::mount_ramfs(
                self.launcher.authorities.root_directory.as_handle_ref(),
                "/tmp",
            )
            .map_err(|_| LaunchError::OperatingSystem)?;
            // Ownership has committed. Formal mount failures
            // are handled here, never by the bootstrap supervisor.
            self.adopt_providers()?;
            let mut storage_available = self.mount_formal_volumes()?;
            if manifest
                .services()
                .any(|service| service.phase() == ServicePhase::System)
            {
                // /data and optional volumes are independent of the libraries
                // needed by Native services. Keep those services available when
                // a later mount fails; fleet provisioning reports its own failure.
                if self.prepare_system_libraries()? {
                    self.launcher.start_phase(
                        manifest,
                        plan,
                        vm_manager_index,
                        &mut self.supervisors,
                        ServicePhase::System,
                    )?;
                } else {
                    storage_available = false;
                    let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                        b"HypeR init: system services unavailable; core services remain supervised\n",
                    );
                }
            }
            self.configure_vm_fleet(manifest, plan, vm_manager_index, storage_available)?;
            self.retire_bootstrap(manifest)?;
            self.supervisors
                .supervise(manifest, &self.launcher.authorities.console)
                .map(|never| match never {})
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
                // Handoff is committed: keep healthy formal/core services alive.
                // Session supervision is a formal service and owns its recovery.
                self.supervisors
                    .supervise(manifest, &self.launcher.authorities.console)
                    .map(|never| match never {})
            }
        }
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
                "/",
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
