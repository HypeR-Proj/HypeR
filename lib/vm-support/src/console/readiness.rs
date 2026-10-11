// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Persistent console subscriptions. Backpressured sources remain disarmed until
//! the worker can service them; source closure always remains observable.

use hyper_os::handle::{ByteChannelObject, HandleRef, VirtualMachineObject};
use hyper_os::vm::DeviceDoorbell;
use hyper_os::wait::{ObjectSignals, RegistrationId, WaitSet};
use hyper_os::{Error, Result, Status};

struct Subscription {
    id: RegistrationId,
    consumed: bool,
}

pub(super) enum Wake {
    Device,
    Input,
    Output,
    Timeout,
    Closed,
}

pub(super) struct Readiness {
    waits: WaitSet,
    subscriptions: [Subscription; 6],
}

impl Readiness {
    pub(super) fn new(
        channel: HandleRef<'_, ByteChannelObject>,
        machine: HandleRef<'_, VirtualMachineObject>,
        route: &DeviceDoorbell,
        virtio_route: &DeviceDoorbell,
    ) -> Result<Self> {
        let waits = WaitSet::new(6)?;
        let ids = [
            route.subscribe(&waits)?,
            virtio_route.subscribe(&waits)?,
            waits.add(channel, ObjectSignals::<ByteChannelObject>::READABLE)?,
            waits.add(channel, ObjectSignals::<ByteChannelObject>::WRITABLE)?,
            waits.add(channel, ObjectSignals::<ByteChannelObject>::PEER_CLOSED)?,
            waits.add(machine, ObjectSignals::<VirtualMachineObject>::TERMINATED)?,
        ];
        Ok(Self {
            waits,
            subscriptions: ids.map(|id| Subscription {
                id,
                consumed: false,
            }),
        })
    }

    pub(super) fn wait(
        &mut self,
        device: bool,
        input: bool,
        output: bool,
        deadline: u64,
    ) -> Result<Wake> {
        // A disarmed source is sampled again by rearm. Readiness changing
        // between the worker's scan and this call therefore cannot be lost.
        for (entry, enabled) in self
            .subscriptions
            .iter_mut()
            .zip([device, true, input, output, true, true])
        {
            if enabled && entry.consumed {
                self.waits.rearm(entry.id)?;
                entry.consumed = false;
            }
        }
        let notification = match self.waits.wait(deadline) {
            Ok(notification) => notification,
            Err(Error::Status(Status::TIMED_OUT)) => return Ok(Wake::Timeout),
            Err(error) => return Err(error),
        };
        let index = self
            .subscriptions
            .iter()
            .position(|entry| entry.id == notification.registration)
            .ok_or(Error::InvalidResponse)?;
        self.subscriptions[index].consumed = true;
        Ok(match index {
            0 | 1 => Wake::Device,
            2 => Wake::Input,
            3 => Wake::Output,
            _ => Wake::Closed,
        })
    }
}
