// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Formal init replaces providers before choosing to retire its predecessor.
//! Provider rendezvous transfer resources; only init stops old processes.

mod process;
mod retirement;

use super::{LaunchError, Runtime};
use core::mem::MaybeUninit;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::handle::*;

const READY: u32 = 0x8008_0020;
const ADOPTED: &[u8] = b"HYPER-SERVICE-ADOPTED/1";
const PROCESS_RIGHTS: Rights = Rights::TRANSFER
    .union(Rights::WAIT)
    .union(Rights::INSPECT)
    .union(Rights::REQUEST_STOP);

pub(super) fn deadline() -> hyper_os::Result<u64> {
    hyper_os::time::deadline_after(std::time::Duration::from_secs(60))
        .map(|deadline| deadline.as_raw())
}

impl Runtime {
    pub(super) fn adopt_providers(&mut self) -> Result<(), LaunchError> {
        self.prepare_system_libraries()?;
        let mut console_adopted = true;
        if let Some(index) = self.providers[0]
            && let Err(error) = self.adopt_console(index)
        {
            console_adopted = false;
            self.retain_unconfirmed_provider(true);
            let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                format!("HypeR init: console adoption failed: {error:?}; retaining the current console\n").as_bytes());
        }
        if !self.bootstrap_storage_ready
            && (self.providers[1].is_some() || self.providers[2].is_some())
        {
            // Failed boot storage cannot be migrated. Formal recovery still
            // gets its own console and services; predecessor owners are kept.
            self.close_unattempted_migrations();
            return Ok(());
        }
        for role in [1, 2] {
            let Some(index) = self.providers[role] else {
                continue;
            };
            let result = if role == 1 {
                self.adopt_io(index)
            } else {
                self.adopt_filesystems(index)
            };
            if let Err(error) = result {
                let state = if self.pending_provider.is_some() {
                    "ownership unconfirmed; both process owners retained for system recovery"
                } else {
                    "replacement preparation failed; bootstrap provider retained"
                };
                self.retain_unconfirmed_provider(false);
                self.close_unattempted_migrations();
                self.bootstrap_storage_ready = false;
                let _ = self.launcher.authorities.console.as_emergency_console().write_all(
                    format!("HypeR init: provider adoption failed: {error:?}; {state}; bootstrap retirement disabled\n").as_bytes());
                return Ok(());
            }
        }
        self.retirement_ready = console_adopted;
        Ok(())
    }

    fn close_unattempted_migrations(&mut self) {
        // A provider not attempted after another one's failure is still
        // waiting for its private descriptor rendezvous. Close that endpoint
        // so it resumes normal recovery administration instead of waiting for
        // a coordinator that has already abandoned this migration attempt.
        self.launcher.authorities.io_handoff.take();
        self.launcher.authorities.filesystem_handoff.take();
    }

    fn adopt_console(&mut self, index: usize) -> hyper_os::Result<()> {
        let (control, controller) = CapabilityChannel::create()?;
        let (logs, connector) = CapabilityChannel::create()?;
        let (ready, reader) = hyper_os::channel::create_pair()?;
        let builder = self.provider_builder("console-raw", "/svc/console-raw", None)?;
        let a = &self.launcher.authorities;
        duplicate(
            &builder,
            a.console.as_handle_ref(),
            hyper_service::console::SYSTEM_CONSOLE.as_raw(),
            Rights::READ.union(Rights::WRITE).union(Rights::WAIT),
        )?;
        move_handle(
            &builder,
            control.into_handle(),
            hyper_service::console::CONTROL.as_raw(),
            Rights::READ.union(Rights::WAIT),
        )?;
        move_handle(
            &builder,
            logs.into_handle(),
            hyper_service::console::LOG_CONNECTOR.as_raw(),
            Rights::READ.union(Rights::WAIT),
        )?;
        move_handle(&builder, ready, READY, Rights::WRITE.union(Rights::WAIT))?;
        self.start_provider(index, builder)?;
        ready_message(&reader, ADOPTED)?;
        self.confirm_provider(index)?;
        self.launcher.authorities.raw_controller = Some(controller.into_handle());
        self.launcher.authorities.log_connector = connector.into_handle();
        self.launcher.authorities.service_output_channel.take();
        Ok(())
    }

    fn adopt_io(&mut self, index: usize) -> hyper_os::Result<()> {
        let endpoint = self
            .launcher
            .authorities
            .io_handoff
            .take()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let channel = CapabilityChannel::from_handle(endpoint);
        let description = receive_description(&channel, b"HYPER-IO-DESCRIBE/2")?;
        let (ready, reader) = hyper_os::channel::create_pair()?;
        let builder = self.provider_builder("io-runtime", "/svc/io-runtime", Some("io-runtime"))?;
        move_handle(
            &builder,
            channel.into_handle(),
            0x8008_0010,
            Rights::READ.union(Rights::WAIT),
        )?;
        move_handle(
            &builder,
            description,
            0x8008_0012,
            Rights::READ.union(Rights::INSPECT),
        )?;
        move_handle(&builder, ready, READY, Rights::WRITE.union(Rights::WAIT))?;
        self.start_provider(index, builder)?;
        ready_message(&reader, b"HYPER-IO-READY/2")?;
        self.confirm_provider(index)
    }

    fn adopt_filesystems(&mut self, index: usize) -> hyper_os::Result<()> {
        let endpoint = self
            .launcher
            .authorities
            .filesystem_handoff
            .take()
            .ok_or(hyper_os::Error::MissingHandle)?;
        let channel = CapabilityChannel::from_handle(endpoint);
        let description = receive_description(&channel, b"HYPER-FS-DESCRIBE/1")?;
        let (ready, reader) = hyper_os::channel::create_pair()?;
        let (owners, sender) = CapabilityChannel::create()?;
        let builder = self.provider_builder("fs-backend", "/svc/fs-backend", Some("fs-backend"))?;
        self.delegate_filesystem_launch(&builder)?;
        move_handle(
            &builder,
            channel.into_handle(),
            0x8008_0011,
            Rights::READ.union(Rights::WRITE).union(Rights::WAIT),
        )?;
        move_handle(
            &builder,
            description,
            0x8008_0013,
            Rights::READ.union(Rights::INSPECT),
        )?;
        move_handle(&builder, ready, READY, Rights::WRITE.union(Rights::WAIT))?;
        move_handle(
            &builder,
            sender.into_handle(),
            0x8008_0021,
            Rights::WRITE.union(Rights::WAIT),
        )?;
        self.start_provider(index, builder)?;
        loop {
            let mut bytes = [MaybeUninit::uninit(); 64];
            let mut slots = [CapabilityReceiveSlot::new::<ProcessObject>(PROCESS_RIGHTS)];
            let message = owners.receive(deadline()?, &mut bytes, &mut slots)?;
            if message.bytes() == b"HYPER-BOOT-OWNERS-END/1" && message.capability_count() == 0 {
                break;
            }
            if message.bytes() != b"HYPER-BOOT-OWNER/1" || message.capability_count() != 1 {
                return Err(hyper_os::Error::InvalidResponse);
            }
            self.retired_processes.push(
                slots[0]
                    .take::<ProcessObject>()?
                    .ok_or(hyper_os::Error::MissingHandle)?,
            );
        }
        ready_message(&reader, ADOPTED)?;
        self.confirm_provider(index)
    }
}

fn receive_description(
    channel: &CapabilityChannel,
    expected: &[u8],
) -> hyper_os::Result<OwnedHandle<VmoObject>> {
    let mut bytes = [MaybeUninit::uninit(); 64];
    let mut slots = [CapabilityReceiveSlot::new::<VmoObject>(
        Rights::READ.union(Rights::TRANSFER).union(Rights::INSPECT),
    )];
    let message = channel.receive(deadline()?, &mut bytes, &mut slots)?;
    if message.bytes() != expected || message.capability_count() != 1 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    slots[0]
        .take::<VmoObject>()?
        .ok_or(hyper_os::Error::MissingHandle)
}

fn ready_message(
    channel: &OwnedHandle<ByteChannelObject>,
    expected: &[u8],
) -> hyper_os::Result<()> {
    use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
    wait_many(
        &[WaitItem::new(
            channel.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )],
        deadline()?,
    )?;
    let mut bytes = [0; 64];
    let length = channel.as_byte_channel().try_receive(&mut bytes)?;
    if &bytes[..length] != expected {
        return Err(hyper_os::Error::InvalidResponse);
    }
    Ok(())
}

fn duplicate<T: TypedObject>(
    builder: &hyper_os::task::ProcessBuilder,
    source: HandleRef<'_, T>,
    purpose: u32,
    rights: Rights,
) -> hyper_os::Result<()> {
    builder.add_handle_duplicate(source, purpose, RightsOffer::Exact(rights))
}
fn move_handle<T: TypedObject>(
    builder: &hyper_os::task::ProcessBuilder,
    source: OwnedHandle<T>,
    purpose: u32,
    rights: Rights,
) -> hyper_os::Result<()> {
    builder
        .add_handle_move(source, purpose, RightsOffer::Exact(rights))
        .map_err(|failure| failure.error())
}
