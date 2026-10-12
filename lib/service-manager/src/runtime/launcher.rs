// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Transactional process construction from a validated launch plan.

use crate::manifest::{LaunchPlan, Manifest, Service, ServicePhase};
use hyper_os::handle::{OwnedHandle, ProcessObject, Rights, RightsOffer};
use hyper_os::startup;
use hyper_os::task::{ProcessBuilder, StartFailure};

use super::LaunchError;
use super::authority::AuthorityInventory;
use super::fleet::VmFleet;
use super::report::report_service_launch_failure;
use super::supervisor::SupervisorSet;

/// Applies validated service declarations to the live authority inventory.
pub(super) struct ServiceLauncher {
    pub(super) authorities: AuthorityInventory,
}

impl ServiceLauncher {
    fn launch_service(
        &mut self,
        service: &Service<'_>,
        service_index: usize,
        vm_manager_index: Option<usize>,
        plan: &LaunchPlan<'_>,
    ) -> Result<OwnedHandle<ProcessObject>, LaunchError> {
        // Each service owns an independent log writer. The raw worker keeps
        // ingress alive across terminal/session restarts.
        self.authorities.service_output_channel = None;
        if service
            .capabilities()
            .any(|grant| grant.source() == "bootstrap.service-output-channel")
        {
            let (writer, reader) =
                hyper_os::channel::create_pair().map_err(LaunchError::service_resources)?;
            hyper_service::console::register_log(
                self.authorities.log_connector.as_handle_ref(),
                service.name(),
                reader,
            )
            .map_err(LaunchError::service_resources)?;
            self.authorities.service_output_channel = Some(writer);
        }
        let executable = self
            .authorities
            .root_directory
            .open(
                service.image(),
                hyper_service::process::EXECUTABLE_FILE_RIGHTS,
            )
            .map_err(LaunchError::service_operation)?;
        let vm_fleet_scope = Some(service_index) == vm_manager_index;
        if vm_fleet_scope && self.authorities.vm_fleet.is_none() {
            self.authorities.vm_fleet = Some(
                VmFleet::new(
                    self.authorities.factory.as_handle_ref(),
                    self.authorities.domain.as_handle_ref(),
                )
                .map_err(LaunchError::service_resources)?,
            );
        }
        let environment = match service.phase() {
            ServicePhase::Boot => &self.authorities.boot_environment,
            ServicePhase::System => self
                .authorities
                .system_environment
                .as_ref()
                .ok_or(LaunchError::InvalidPlan)?,
        };
        // Retain a local fixed scope before offers consume inventory fields.
        let environment = hyper_service::process::open_environment(environment, "/")
            .map_err(LaunchError::service_resources)?;
        let loader = hyper_service::process::open_loader(&environment, service.loader())
            .map_err(LaunchError::service_operation)?;
        let builder = ProcessBuilder::create(
            self.authorities.factory.as_handle_ref(),
            if vm_fleet_scope {
                self.authorities.vm_fleet()?.group.as_handle_ref()
            } else {
                self.authorities.group.as_handle_ref()
            },
            if vm_fleet_scope {
                self.authorities.vm_fleet()?.domain.as_handle_ref()
            } else {
                self.authorities.domain.as_handle_ref()
            },
            executable.as_handle_ref(),
            loader.as_handle_ref(),
        )
        .map_err(LaunchError::service_operation)?;
        builder
            .set_name(service.name())
            .map_err(LaunchError::service_resources)?;
        builder
            .add_argument(service.image())
            .map_err(LaunchError::service_resources)?;
        builder
            .add_handle_duplicate(
                self.authorities.library_directory.as_handle_ref(),
                startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
                RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE).union(Rights::PROTECT)),
            )
            .map_err(LaunchError::service_resources)?;

        for (capability_index, _) in service.capabilities().enumerate() {
            let grant = plan
                .capability_grant(service_index, capability_index)
                .ok_or(LaunchError::InvalidPlan)?;
            self.authorities
                .offer(grant, vm_fleet_scope, &builder, &loader, &environment)?;
        }

        builder.seal().map_err(LaunchError::service_image)?;
        builder.start().map_err(|failure| {
            let (error, committed) = match failure {
                StartFailure::Rejected { error, .. } => (error, false),
                StartFailure::Committed(error) => (error, true),
            };
            let message = format!(
                "HypeR init: service '{}' {} failed: {error:?}\n",
                service.name(),
                if committed { "loader" } else { "start" }
            );
            let _ = self
                .authorities
                .console
                .as_emergency_console()
                .write_all(message.as_bytes());
            if committed {
                LaunchError::service_image(error)
            } else {
                LaunchError::service_resources(error)
            }
        })
    }
}

impl ServiceLauncher {
    pub(super) fn start_phase(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan<'_>,
        vm_manager_index: Option<usize>,
        supervisors: &mut SupervisorSet,
        phase: ServicePhase,
    ) -> Result<(), LaunchError> {
        for position in 0..plan.service_count() {
            let service_index = plan
                .service_index(position)
                .ok_or(LaunchError::InvalidPlan)?;
            let service = manifest
                .service(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            if service.phase() != phase {
                continue;
            }
            if supervisors
                .processes
                .get(service_index)
                .ok_or(LaunchError::InvalidPlan)?
                .is_some()
            {
                return Err(LaunchError::InvalidPlan);
            }
            let supervisor =
                match self.launch_service(service, service_index, vm_manager_index, plan) {
                    Ok(supervisor) => supervisor,
                    Err(error) => {
                        if Some(service_index) == vm_manager_index {
                            self.authorities.close_unstarted_vm_manager();
                        }
                        report_service_launch_failure(
                            &self.authorities.console,
                            service.name(),
                            &error,
                        );
                        if !service.critical() && matches!(error, LaunchError::ServiceUnavailable) {
                            continue;
                        }
                        return Err(error);
                    }
                };
            let slot = supervisors
                .processes
                .get_mut(service_index)
                .ok_or(LaunchError::InvalidPlan)?;
            *slot = Some(supervisor);
        }
        Ok(())
    }
}
