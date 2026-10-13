// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Retirement policy belongs entirely to formal init, after replacement readiness.
use super::{LaunchError, Runtime, deadline, ready_message};
use crate::manifest::Manifest;
use hyper_os::{handle::*, task::ProcessPhase};

impl Runtime {
    pub(in crate::runtime) fn retire_bootstrap(
        &mut self,
        manifest: &Manifest<'_>,
    ) -> Result<(), LaunchError> {
        if !self.retirement_ready {
            return Ok(());
        }
        if let Err(error) = self.try_retire_bootstrap(manifest) {
            let _ = self
                .launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(
                    format!("HypeR init: bootstrap retirement deferred: {error:?}\n").as_bytes(),
                );
        }
        Ok(())
    }

    fn try_retire_bootstrap(&mut self, manifest: &Manifest<'_>) -> hyper_os::Result<()> {
        // A configured core service without a migration protocol remains a
        // supervised dependency. Inspect every such owner before changing the
        // supervisor set or stopping any predecessor, including the old group.
        if let Some(index) = retirement_blocker(self.core_count, &self.providers, |index| {
            let service = manifest
                .service(index)
                .ok_or(hyper_os::Error::InvalidResponse)?;
            self.supervisors.processes[index]
                .as_ref()
                .map(|process| {
                    process
                        .as_process_supervisor()
                        .info()
                        .map(|info| (service.critical(), info.phase))
                })
                .transpose()
        })? {
            let service = manifest
                .service(index)
                .ok_or(hyper_os::Error::InvalidResponse)?;
            let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                format!(
                    "HypeR init: bootstrap retirement deferred: core service '{}' has no replacement\n",
                    service.name(),
                )
                .as_bytes(),
            );
            return Ok(());
        }
        // Stop explicit old providers first so their independent log writers
        // close. The old raw worker drains their final records before stopping.
        let until = deadline()?;
        for index in 0..self.core_count {
            if !self.providers.contains(&Some(index))
                && let Some(old) = self.supervisors.processes[index].take()
            {
                self.retired_processes.push(old);
            }
        }
        for process in &self.retired_processes {
            process.as_process_supervisor().request_stop()?;
        }
        for process in &self.retired_processes {
            wait_retired(process, until)?;
        }
        self.retired_processes.clear();
        self.launcher.authorities.service_output_channel.take();
        if let Some(channel) = self.launcher.authorities.console_drain.as_ref() {
            channel.as_byte_channel().send(b"HYPER-CONSOLE-DRAIN/1")?;
            ready_message(channel, b"HYPER-CONSOLE-DRAINED/1")?;
        }
        self._boot_group.as_task_group().request_stop()?;
        if let Some(console) = self.old_console.as_ref() {
            wait_retired(console, until)?;
        }
        self.old_console.take();
        self.launcher.authorities.console_drain.take();
        self.launcher.authorities.boot_environment.take();
        let Some(path) = self.retirement_path.as_deref() else {
            return Ok(());
        };
        loop {
            match hyper_os::filesystem::unmount(
                self.launcher.authorities.root_directory.as_handle_ref(),
                path,
            ) {
                Ok(()) => break,
                Err(hyper_os::Error::Status(hyper_os::Status::BUSY)) if before(until)? => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => return Err(error),
            }
        }
        let _ = self
            .launcher
            .authorities
            .console
            .as_emergency_console()
            .write_all(b"HypeR init: bootstrap retired; boot archive detached\n");
        self.retirement_ready = false;
        Ok(())
    }
}

/// Only completed, noncritical one-shots may retire without a replacement.
/// Stopped is quiescent and cannot admit threads again; `wait_retired` still
/// joins its resource teardown. Critical exits stay visible to supervision.
fn retirement_blocker(
    core_count: usize,
    providers: &[Option<usize>; 3],
    mut inspect: impl FnMut(usize) -> hyper_os::Result<Option<(bool, ProcessPhase)>>,
) -> hyper_os::Result<Option<usize>> {
    for index in 0..core_count {
        if providers.contains(&Some(index)) {
            continue;
        }
        if let Some((critical, phase)) = inspect(index)?
            && (critical
                || !matches!(
                    phase,
                    ProcessPhase::Stopped | ProcessPhase::Retiring | ProcessPhase::Retired
                ))
        {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn before(deadline: u64) -> hyper_os::Result<bool> {
    Ok(hyper_os::time::monotonic_now()?.as_nanoseconds() < deadline)
}
fn wait_retired(process: &OwnedHandle<ProcessObject>, deadline: u64) -> hyper_os::Result<()> {
    while process.as_process_supervisor().info()?.phase != ProcessPhase::Retired {
        if !before(deadline)? {
            return Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/retirement.rs"]
mod tests;
