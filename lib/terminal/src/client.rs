// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scoped control client and optional input independent of redirected stdin.

use core::mem::MaybeUninit;
use hyper_os::capability_channel::{
    CapabilityChannel, CapabilityDisposition, CapabilityReceiveSlot,
};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, Rights, RightsOffer,
};
use hyper_os::startup::{Startup, StartupPurpose};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error, Result, Status};

use crate::Settings;
use crate::protocol::{self, Header};

pub const CONTROL: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8009_0001);
pub const MANAGEMENT: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8009_0002);
pub const READY: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8009_0003);
pub const EVENTS: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8009_0004);
pub const RECOVERY: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8009_0005);
pub const RAW_INPUT: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8009_0006);
pub const RAW_OUTPUT: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8009_0007);
/// Foreground terminal stream, independent of stdin (for example in a pager).
pub const STREAM: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8009_0008);
pub const CONTROL_RIGHTS: Rights = Rights::READ
    .union(Rights::WRITE)
    .union(Rights::WAIT)
    .union(Rights::TRANSFER)
    .union(Rights::DUPLICATE);
pub const EVENT_RIGHTS: Rights = Rights::READ.union(Rights::WAIT).union(Rights::TRANSFER);
pub const OBSERVER_RIGHTS: Rights = Rights::WAIT.union(Rights::TRANSFER);

/// Prepared endpoints carry no input/configuration authority until the launcher
/// binds the returned Process observer. Dropping them cannot consume typeahead.
/// The launcher releases the id on failed launch; optional control-handle
/// closure does not release a Process-owned lease or its bounded session slot.
pub struct PreparedClient {
    pub id: u64,
    pub stream: OwnedHandle<CapabilityChannelObject>,
    pub control: OwnedHandle<CapabilityChannelObject>,
    pub events: OwnedHandle<ByteChannelObject>,
}

pub struct Terminal {
    control: CapabilityChannel,
    next_id: u64,
    events: Option<OwnedHandle<ByteChannelObject>>,
    stream: Option<OwnedHandle<CapabilityChannelObject>>,
}

impl Terminal {
    #[must_use]
    pub fn from_handle(handle: OwnedHandle<CapabilityChannelObject>) -> Self {
        Self {
            control: CapabilityChannel::from_handle(handle),
            next_id: 1,
            events: None,
            stream: None,
        }
    }

    pub fn take(startup: &mut Startup<'_>) -> Result<Option<Self>> {
        let Some(handle) = startup.take_optional(CONTROL)? else {
            return Ok(None);
        };
        let mut terminal = Self::from_handle(handle);
        terminal.events = startup.take_optional(EVENTS)?;
        terminal.stream = startup.take_optional(STREAM)?;
        Ok(Some(terminal))
    }

    /// Opens keyboard input without consuming the application's stdin. The
    /// same foreground lease governs both connections and all control changes.
    pub fn open_input(&self) -> Result<hyper_os::stream::Connection> {
        hyper_os::stream::connect(
            self.stream
                .as_ref()
                .ok_or(Error::MissingHandle)?
                .as_handle_ref(),
            hyper_os::stream::ACCESS_READ,
        )
    }

    pub fn prepare_child(&mut self) -> Result<PreparedClient> {
        let mut slots = [
            CapabilityReceiveSlot::new::<CapabilityChannelObject>(
                hyper_os::stream::CONNECTOR_RIGHTS,
            ),
            CapabilityReceiveSlot::new::<CapabilityChannelObject>(CONTROL_RIGHTS),
            CapabilityReceiveSlot::new::<ByteChannelObject>(EVENT_RIGHTS),
        ];
        let (reply, _) = self.request(protocol::PREPARE, 0, &[], &mut [], &mut slots)?;
        Ok(PreparedClient {
            id: reply.value,
            stream: slots[0].take()?.ok_or(Error::MissingHandle)?,
            control: slots[1].take()?.ok_or(Error::MissingHandle)?,
            events: slots[2].take()?.ok_or(Error::MissingHandle)?,
        })
    }

    /// Transfers observation only, then atomically activates the child's lease.
    /// The caller keeps its supervisor and therefore its independent stop right.
    pub fn bind_child(&mut self, id: u64, observer: OwnedHandle<ProcessObject>) -> Result<()> {
        let mut owner = Some(observer);
        let mut offers = [CapabilityDisposition::move_handle(
            &mut owner,
            RightsOffer::Exact(OBSERVER_RIGHTS),
        )?];
        self.request(protocol::BIND, id, &[], &mut offers, &mut [])?;
        Ok(())
    }

    pub fn release_child(&mut self, id: u64) -> Result<()> {
        self.request(protocol::RELEASE, id, &[], &mut [], &mut [])?;
        Ok(())
    }

    pub fn settings(&mut self) -> Result<Settings> {
        let (_, bytes) = self.request(protocol::GET_SETTINGS, 0, &[], &mut [], &mut [])?;
        protocol::decode_settings(&bytes).ok_or(Error::InvalidResponse)
    }

    /// Applies after previously acknowledged writes. Applications must first
    /// flush their own buffered writer; this does not drain the physical UART.
    pub fn set_settings(&mut self, settings: Settings) -> Result<()> {
        settings.validate().map_err(|_| Error::InvalidResponse)?;
        self.request(
            protocol::SET_SETTINGS,
            0,
            &protocol::encode_settings(settings),
            &mut [],
            &mut [],
        )?;
        Ok(())
    }

    pub fn size(&mut self) -> Result<(u16, u16)> {
        let (reply, _) = self.request(protocol::GET_SIZE, 0, &[], &mut [], &mut [])?;
        Ok((reply.value as u16, (reply.value >> 16) as u16))
    }

    pub fn set_size(&mut self, columns: u16, rows: u16) -> Result<()> {
        if columns == 0 || rows == 0 {
            return Err(Error::InvalidResponse);
        }
        self.request(
            protocol::SET_SIZE,
            u64::from(columns) | u64::from(rows) << 16,
            &[],
            &mut [],
            &mut [],
        )?;
        Ok(())
    }

    /// Returns coalesced event flags; the separate event endpoint is a wake hint.
    pub fn take_events(&mut self) -> Result<crate::EventFlags> {
        if let Some(events) = &self.events {
            let mut bytes = [0; 4];
            loop {
                match events.as_byte_channel().try_receive(&mut bytes) {
                    Ok(4) => {}
                    Err(Error::Status(Status::WOULD_BLOCK)) => break,
                    Err(error) => return Err(error),
                    _ => return Err(Error::InvalidResponse),
                }
            }
        }
        let (reply, _) = self.request(protocol::TAKE_EVENTS, 0, &[], &mut [], &mut [])?;
        Ok(crate::EventFlags(reply.value as u32))
    }

    /// A level wake hint for resize, input loss, mode and foreground changes.
    /// Call `take_events` after observing it; byte input has its own pending read.
    pub fn event_wait_item(&self) -> Option<WaitItem<'_>> {
        self.events.as_ref().map(|events| {
            WaitItem::new(
                events.as_handle_ref(),
                ObjectSignals::<ByteChannelObject>::READABLE
                    .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
            )
        })
    }

    pub fn clear_input(&mut self) -> Result<()> {
        self.request(protocol::CLEAR_INPUT, 0, &[], &mut [], &mut [])?;
        Ok(())
    }

    pub fn pause_output(&mut self, paused: bool) -> Result<()> {
        self.request(
            protocol::PAUSE_OUTPUT,
            u64::from(paused),
            &[],
            &mut [],
            &mut [],
        )?;
        Ok(())
    }

    /// Tracks the supported fullscreen state so owner death restores a baseline.
    pub fn presentation(&mut self, alternate_screen: bool, hidden_cursor: bool) -> Result<()> {
        self.request(
            protocol::PRESENTATION,
            u64::from(alternate_screen) | u64::from(hidden_cursor) << 1,
            &[],
            &mut [],
            &mut [],
        )?;
        Ok(())
    }

    fn request(
        &mut self,
        operation: u16,
        value: u64,
        payload: &[u8],
        offers: &mut [CapabilityDisposition<'_>],
        slots: &mut [CapabilityReceiveSlot],
    ) -> Result<(Header, [u8; protocol::SETTINGS_BYTES])> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(Error::InvalidResponse)?;
        let mut request = [0; protocol::MAX_MESSAGE];
        request[..protocol::HEADER_BYTES].copy_from_slice(
            &Header {
                operation,
                id,
                value,
                status: 0,
            }
            .encode(),
        );
        request
            .get_mut(protocol::HEADER_BYTES..protocol::HEADER_BYTES + payload.len())
            .ok_or(Error::InvalidResponse)?
            .copy_from_slice(payload);
        loop {
            match self
                .control
                .try_send(&request[..protocol::HEADER_BYTES + payload.len()], offers)
            {
                Ok(()) => break,
                Err(Error::Status(Status::WOULD_BLOCK)) => {
                    wait_many(
                        &[WaitItem::new(
                            self.control.as_handle_ref(),
                            ObjectSignals::<CapabilityChannelObject>::PEER_RECEIVING
                                .union(ObjectSignals::<CapabilityChannelObject>::PEER_CLOSED),
                        )],
                        hyper_os::DEADLINE_INFINITE,
                    )?;
                }
                Err(error) => return Err(error),
            }
        }
        let mut response = [MaybeUninit::uninit(); protocol::MAX_MESSAGE];
        let message = self
            .control
            .receive(hyper_os::DEADLINE_INFINITE, &mut response, slots)?;
        let reply = Header::decode(message.bytes()).ok_or(Error::InvalidResponse)?;
        if reply.operation != operation | protocol::RESPONSE || reply.id != id {
            return Err(Error::InvalidResponse);
        }
        let status = Status::from_raw(reply.status);
        if status != Status::OK {
            return Err(Error::Status(status));
        }
        let expected_payload = if operation == protocol::GET_SETTINGS {
            protocol::SETTINGS_BYTES
        } else {
            0
        };
        if message.bytes().len() != protocol::HEADER_BYTES + expected_payload
            || message.capability_count() != slots.len()
        {
            return Err(Error::InvalidResponse);
        }
        let mut payload = [0; protocol::SETTINGS_BYTES];
        payload[..expected_payload].copy_from_slice(&message.bytes()[protocol::HEADER_BYTES..]);
        Ok((reply, payload))
    }
}
