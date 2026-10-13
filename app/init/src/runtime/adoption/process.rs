// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Resident replacement construction while predecessor filesystem workers run.
use super::{Runtime, duplicate};
use hyper_os::{
    handle::*,
    startup,
    task::{ProcessBuilder, StartFailure},
};
use hyper_service::process as contract;

impl Runtime {
    pub(super) fn provider_builder(
        &self,
        name: &str,
        path: &str,
        log: Option<&str>,
    ) -> hyper_os::Result<ProcessBuilder> {
        let a = &self.launcher.authorities;
        let environment = a
            .system_environment
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let image = a
            .root_directory
            .open(path, contract::EXECUTABLE_FILE_RIGHTS)?;
        let loader = contract::open_loader(environment, contract::LoaderKind::Dynamic)?;
        let builder = ProcessBuilder::create(
            a.factory.as_handle_ref(),
            a.group.as_handle_ref(),
            a.domain.as_handle_ref(),
            image.as_handle_ref(),
            loader.as_handle_ref(),
        )?;
        builder.set_name(name)?;
        builder.add_argument(path)?;
        builder.set_memory_lock_allowed(true)?;
        duplicate(
            &builder,
            a.library_directory.as_handle_ref(),
            startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
            Rights::READ.union(Rights::EXECUTE).union(Rights::PROTECT),
        )?;
        if let Some(name) = log {
            let (writer, reader) = hyper_os::channel::create_pair()?;
            hyper_service::console::register_log(a.log_connector.as_handle_ref(), name, reader)?;
            for purpose in [
                hyper_service::stdio::STANDARD_OUTPUT.as_raw(),
                hyper_service::stdio::STANDARD_ERROR.as_raw(),
            ] {
                duplicate(
                    &builder,
                    writer.as_handle_ref(),
                    purpose,
                    Rights::WRITE
                        .union(Rights::WAIT)
                        .union(Rights::DUPLICATE)
                        .union(Rights::TRANSFER),
                )?;
            }
        }
        Ok(builder)
    }

    pub(super) fn delegate_filesystem_launch(
        &self,
        builder: &ProcessBuilder,
    ) -> hyper_os::Result<()> {
        let a = &self.launcher.authorities;
        let environment = a
            .system_environment
            .as_ref()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let loader = contract::open_loader(environment, contract::LoaderKind::Dynamic)?;
        duplicate(
            builder,
            a.factory.as_handle_ref(),
            startup::TASK_FACTORY.as_raw(),
            Rights::CREATE_PROCESS
                .union(Rights::LOCK_MEMORY)
                .union(Rights::CREATE_TASK_GROUP)
                .union(Rights::DUPLICATE)
                .union(Rights::TRANSFER),
        )?;
        duplicate(
            builder,
            a.domain.as_handle_ref(),
            startup::RESOURCE_DOMAIN.as_raw(),
            Rights::CREATE_RESOURCE_DOMAIN
                .union(Rights::RESOURCE_DOMAIN_SPONSOR)
                .union(Rights::DUPLICATE)
                .union(Rights::TRANSFER),
        )?;
        duplicate(
            builder,
            a.group.as_handle_ref(),
            startup::TASK_GROUP.as_raw(),
            Rights::TASK_GROUP_ATTACH_PROCESS
                .union(Rights::DUPLICATE)
                .union(Rights::TRANSFER),
        )?;
        duplicate(
            builder,
            a.root_directory.as_handle_ref(),
            startup::ROOT_DIRECTORY.as_raw(),
            a.root_directory.as_handle_ref().info()?.rights,
        )?;
        duplicate(
            builder,
            environment.as_handle_ref(),
            contract::ENVIRONMENT_DIRECTORY.as_raw(),
            contract::ENVIRONMENT_DIRECTORY_CONTRACT.required_rights(),
        )?;
        duplicate(
            builder,
            loader.as_handle_ref(),
            startup::CHILD_LOADER.as_raw(),
            contract::CHILD_LOADER_CONTRACT.required_rights(),
        )?;
        duplicate(
            builder,
            a.library_directory.as_handle_ref(),
            contract::CHILD_LIBRARY_DIRECTORY.as_raw(),
            contract::CHILD_LIBRARY_DIRECTORY_CONTRACT.required_rights(),
        )
    }

    pub(super) fn start_provider(
        &mut self,
        index: usize,
        builder: ProcessBuilder,
    ) -> hyper_os::Result<()> {
        if index >= self.supervisors.processes.len() || self.pending_provider.is_some() {
            return Err(hyper_os::Error::InvalidResponse);
        }
        builder.seal()?;
        let candidate = builder.start().map_err(|failure| match failure {
            StartFailure::Rejected { error, .. } | StartFailure::Committed(error) => error,
        })?;
        self.pending_provider = Some((index, candidate));
        Ok(())
    }

    /// READY confirms ownership. Only now may this process take the original
    /// service's critical-supervision slot, with the predecessor held for retire.
    pub(super) fn confirm_provider(&mut self, index: usize) -> hyper_os::Result<()> {
        if self
            .pending_provider
            .as_ref()
            .is_none_or(|(pending, _)| *pending != index)
        {
            return Err(hyper_os::Error::InvalidResponse);
        }
        let (_, candidate) = self
            .pending_provider
            .take()
            .ok_or(hyper_os::Error::MissingHandle)?;
        if let Some(old) = self.supervisors.processes[index].replace(candidate) {
            if Some(index) == self.providers[0] {
                self.old_console = Some(old);
            } else {
                self.retired_processes.push(old);
            }
        }
        self.supervisors.adoption_recovery[index] = false;
        Ok(())
    }

    pub(super) fn retain_unconfirmed_provider(&mut self, independent: bool) {
        if let Some((pending, candidate)) = self.pending_provider.take() {
            if independent {
                // Raw console owns fresh endpoints only. It cannot have taken
                // resources from its predecessor, so its stop is rollback-safe.
                let _ = candidate.as_process_supervisor().request_stop();
            } else {
                // A timeout does not prove that MOVE failed. The old process
                // may be parked without resources, and the candidate may still
                // be completing adoption. Preserve both without claiming rollback.
                self.supervisors.adoption_recovery[pending] = true;
            }
            self.unconfirmed_providers.push(candidate);
        }
    }
}
