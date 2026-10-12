// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Service process supervision during startup handshakes and steady operation.

use std::convert::Infallible;

use crate::manifest::{MAX_SERVICES, Manifest};
use crate::supervision::{self, TerminationAction};
use hyper_os::handle::{ConsoleObject, OwnedHandle, ProcessObject};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};

use super::LaunchError;
use super::report::report_service_termination;

const _: () = assert!(MAX_SERVICES < hyper_os::wait::MAX_ITEMS);

/// Stable service-index ownership and wait-set mapping.
pub(super) struct SupervisorSet {
    pub(super) processes: [Option<OwnedHandle<ProcessObject>>; MAX_SERVICES],
}

impl SupervisorSet {
    /// Waits for one startup endpoint while continuing to supervise every
    /// service. The endpoint's provider must remain alive until completion.
    pub(super) fn wait_for_service_event(
        &mut self,
        manifest: &Manifest<'_>,
        required_service: usize,
        event: WaitItem<'_>,
        deadline: u64,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<u64, LaunchError> {
        let provider = manifest
            .service(required_service)
            .ok_or(LaunchError::InvalidPlan)?;
        if self
            .processes
            .get(required_service)
            .and_then(Option::as_ref)
            .is_none()
        {
            return Err(if provider.critical() {
                LaunchError::CriticalServiceTerminated
            } else {
                LaunchError::RequiredServiceUnavailable
            });
        }
        loop {
            let selected = {
                let mut items = Vec::with_capacity(MAX_SERVICES + 1);
                let mut services = Vec::with_capacity(MAX_SERVICES);
                // Critical exits must win even when an optional provider exits
                // or sends its readiness result in the same observation.
                for critical in [true, false] {
                    for (index, process) in self.processes.iter().enumerate() {
                        if let Some(process) = process {
                            let service =
                                manifest.service(index).ok_or(LaunchError::InvalidPlan)?;
                            if service.critical() != critical {
                                continue;
                            }
                            items.push(WaitItem::new(
                                process.as_handle_ref(),
                                ObjectSignals::<ProcessObject>::TERMINATED,
                            ));
                            services.push(index);
                        }
                    }
                }
                // Process events precede endpoint readiness, so a queued
                // successful reply cannot hide an observed critical failure.
                items.push(event);
                let observation = wait_many(&items, deadline).map_err(|error| match error {
                    hyper_os::Error::Status(hyper_os::Status::TIMED_OUT) => {
                        LaunchError::WaitTimedOut
                    }
                    _ => LaunchError::OperatingSystem,
                })?;
                if observation.index == services.len() {
                    return Ok(observation.observed);
                }
                *services
                    .get(observation.index)
                    .ok_or(LaunchError::InvalidPlan)?
            };
            match self.observe_service_termination(
                manifest,
                selected,
                Some(required_service),
                console,
            )? {
                TerminationAction::Continue => {}
                TerminationAction::ProviderUnavailable => {
                    return Err(LaunchError::RequiredServiceUnavailable);
                }
                TerminationAction::FailSystem => {
                    return Err(LaunchError::CriticalServiceTerminated);
                }
            }
        }
    }

    pub(super) fn supervise(
        &mut self,
        manifest: &Manifest<'_>,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<Infallible, LaunchError> {
        loop {
            let selected = {
                let mut items = Vec::with_capacity(MAX_SERVICES);
                let mut services = Vec::with_capacity(MAX_SERVICES);
                for (index, process) in self.processes.iter().enumerate() {
                    if let Some(process) = process {
                        items.push(WaitItem::new(
                            process.as_handle_ref(),
                            ObjectSignals::<ProcessObject>::TERMINATED,
                        ));
                        services.push(index);
                    }
                }
                let observation = wait_many(&items, hyper_os::DEADLINE_INFINITE)
                    .map_err(|_| LaunchError::OperatingSystem)?;
                *services
                    .get(observation.index)
                    .ok_or(LaunchError::InvalidPlan)?
            };
            if self.observe_service_termination(manifest, selected, None, console)?
                == TerminationAction::FailSystem
            {
                return Err(LaunchError::CriticalServiceTerminated);
            }
        }
    }

    fn observe_service_termination(
        &mut self,
        manifest: &Manifest<'_>,
        index: usize,
        required_service: Option<usize>,
        console: &OwnedHandle<ConsoleObject>,
    ) -> Result<TerminationAction, LaunchError> {
        let service = manifest.service(index).ok_or(LaunchError::InvalidPlan)?;
        let process = self
            .processes
            .get_mut(index)
            .and_then(Option::take)
            .ok_or(LaunchError::InvalidPlan)?;
        report_service_termination(console, service.name(), service.critical(), &process);
        Ok(supervision::startup_termination_action(
            service.critical(),
            required_service == Some(index),
        ))
    }
}
