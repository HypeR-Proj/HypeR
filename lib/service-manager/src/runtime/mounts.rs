// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Formal mount policy runs only after init has adopted core supervision and
//! switched roots. The bootstrap filesystem manager retains all block owners.

use hyper_os::capability_channel::CapabilityChannel;
use hyper_os::handle::Rights;
use hyper_service::filesystem::control;

use super::{LaunchError, Runtime};

impl Runtime {
    pub(super) fn mount_formal_volumes(&self) -> Result<bool, LaunchError> {
        let result = (|| -> Result<(), String> {
            // An empty table needs no block service. This also permits entirely
            // RAM-resident systems and diskless recovery deployments.
            let info = std::fs::metadata(control::SYSTEM_FSTAB)
                .map_err(|error| format!("{}: {error}", control::SYSTEM_FSTAB))?;
            if !info.is_file() {
                return Err(format!("{} is not a regular file", control::SYSTEM_FSTAB));
            }
            if info.len() == 0 {
                return Ok(());
            }
            if !self.bootstrap_storage_ready {
                return Err("bootstrap storage was unavailable".into());
            }
            let owner = self
                .launcher
                .authorities
                .filesystem_control_client
                .as_ref()
                .ok_or("filesystem manager is not configured")?;
            let connector = CapabilityChannel::from_handle(
                owner
                    .duplicate(Rights::WAIT.union(Rights::WRITE))
                    .map_err(|error| format!("filesystem connector: {error}"))?,
            );
            let request = format!("all\0{}\0", control::SYSTEM_FSTAB);
            let (ok, reply) = control::execute_channel(&connector, request.as_bytes())
                .map_err(|error| format!("filesystem manager: {error}"))?;
            if !ok {
                return Err(reply);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.launcher
                .authorities
                .console
                .as_emergency_console()
                .write_all(format!("HypeR init: formal mounts unavailable: {error}\n").as_bytes())
                .map_err(|_| LaunchError::OperatingSystem)?;
            return Ok(false);
        }
        Ok(true)
    }
}
