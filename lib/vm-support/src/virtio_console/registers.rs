// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::queue::SplitQueue;
use super::{DRIVER_OK, Device, Error, FAILED, NEEDS_RESET, QUEUE_MAX, VERSION_1};

const ACKNOWLEDGE: u32 = 1;
const DRIVER: u32 = 2;
const FEATURES_OK: u32 = 8;

impl Device {
    pub fn read(&self, offset: u64, width: u8) -> Result<u64, Error> {
        // None of SIZE, MULTIPORT or EMERG_WRITE is offered. Their configuration
        // fields have no active behavior; ordinary byte/halfword reads are legal.
        if offset >= 0x100 {
            return if matches!(width, 1 | 2 | 4)
                && offset.is_multiple_of(u64::from(width))
                && offset < 0x10c
            {
                Ok(0)
            } else {
                Err(Error::InvalidAccess)
            };
        }
        validate_access(offset, width, 0)?;
        let queue = self.queues.get(self.queue_selector as usize);
        let value = match offset {
            0x000 => 0x7472_6976,
            0x004 => 2,
            0x008 => 3,
            0x00c => 0,
            0x010 if self.device_selector < 2 => (VERSION_1 >> (32 * self.device_selector)) as u32,
            0x010 => 0,
            0x034 => queue.map_or(0, |_| u32::from(QUEUE_MAX)),
            0x044 => u32::from(queue.is_some_and(|queue| queue.config.ready)),
            0x060 => self.interrupt_status,
            0x070 => self.status,
            0x0b0 | 0x0b4 => u32::MAX,
            0x0b8 | 0x0bc | 0x0fc => 0,
            _ => return Err(Error::InvalidAccess),
        };
        Ok(u64::from(value))
    }

    pub fn write(&mut self, offset: u64, width: u8, value: u64) -> Result<(), Error> {
        validate_access(offset, width, value)?;
        let value = value as u32;
        match offset {
            0x014 => self.device_selector = value,
            0x024 => self.driver_selector = value,
            0x020 => self.set_features(value)?,
            0x030 => self.queue_selector = value,
            0x038 | 0x044 | 0x080 | 0x084 | 0x090 | 0x094 | 0x0a0 | 0x0a4 => {
                self.configure_queue(offset, value)?;
            }
            0x050 => {
                // Notification is only a prompt: the worker scans durable ring
                // indices after completing MMIO, so no kick counter can wrap or
                // race a reset. Invalid queue numbers grant no memory access.
                if value >= self.queues.len() as u32 {
                    return Err(Error::InvalidQueue);
                }
            }
            0x064 => self.interrupt_status &= !(value & 3),
            0x070 => self.set_status(value)?,
            0x0ac => {} // No shared-memory capability regions.
            _ => return Err(Error::InvalidAccess),
        }
        Ok(())
    }

    fn set_features(&mut self, value: u32) -> Result<(), Error> {
        if self.status & DRIVER == 0
            || self.status & (FEATURES_OK | DRIVER_OK | NEEDS_RESET | FAILED) != 0
        {
            return Err(Error::InvalidStatus);
        }
        if self.driver_selector < 2 {
            let shift = 32 * self.driver_selector;
            self.features =
                (self.features & !(u64::from(u32::MAX) << shift)) | (u64::from(value) << shift);
        } else if value != 0 {
            return Err(Error::InvalidAccess);
        }
        Ok(())
    }

    fn configure_queue(&mut self, offset: u64, value: u32) -> Result<(), Error> {
        let index = self.queue_selector as usize;
        let mut config = self.queues.get(index).ok_or(Error::InvalidQueue)?.config;
        if offset == 0x044 && value == 0 {
            if config.ready && self.status & DRIVER_OK != 0 {
                // QueueReady=0 quiesces immediately: this worker is the only
                // accessor. Independent queue restart is not advertised.
                self.status |= NEEDS_RESET;
                self.interrupt_status |= 2;
            }
            self.queues[index] = SplitQueue::default();
            return Ok(());
        }
        if self.status & FEATURES_OK == 0 || self.status & (DRIVER_OK | NEEDS_RESET | FAILED) != 0 {
            return Err(Error::InvalidStatus);
        }
        if config.ready {
            return Err(Error::InvalidQueue);
        }
        match offset {
            0x038 => config.size = value,
            0x044 if value == 1 => {
                config.validate(self.memory_base..self.memory_end)?;
                let existing = self.ring_regions();
                if config.regions().iter().any(|(base, len, _)| {
                    existing
                        .iter()
                        .any(|(start, end)| *base < *end && *start < *base + *len)
                }) {
                    return Err(Error::InvalidQueue);
                }
                config.ready = true;
            }
            0x044 => return Err(Error::InvalidQueue),
            0x080 => set_half(&mut config.descriptor, false, value),
            0x084 => set_half(&mut config.descriptor, true, value),
            0x090 => set_half(&mut config.available, false, value),
            0x094 => set_half(&mut config.available, true, value),
            0x0a0 => set_half(&mut config.used, false, value),
            0x0a4 => set_half(&mut config.used, true, value),
            _ => return Err(Error::InvalidAccess),
        }
        self.queues[index].config = config;
        Ok(())
    }

    fn set_status(&mut self, value: u32) -> Result<(), Error> {
        if value == 0 {
            self.queues = [SplitQueue::default(), SplitQueue::default()];
            self.device_selector = 0;
            self.driver_selector = 0;
            self.queue_selector = 0;
            self.features = 0;
            self.status = 0;
            self.interrupt_status = 0;
            return Ok(());
        }
        let driver_bits = ACKNOWLEDGE | DRIVER | DRIVER_OK | FEATURES_OK | FAILED;
        if value & !(driver_bits | NEEDS_RESET) != 0
            || value & self.status != self.status
            || (value & NEEDS_RESET != 0 && self.status & NEEDS_RESET == 0)
            || value & ACKNOWLEDGE == 0
            || (value & FEATURES_OK != 0 && value & DRIVER == 0)
            || (value & DRIVER_OK != 0 && self.status & FEATURES_OK == 0)
        {
            return Err(Error::InvalidStatus);
        }
        if value & FEATURES_OK != 0 && self.features != VERSION_1 {
            self.status = value & !(FEATURES_OK | DRIVER_OK);
            return Ok(());
        }
        if value & DRIVER_OK != 0
            && self.status & DRIVER_OK == 0
            && (value & FAILED != 0
                || self.status & (FAILED | NEEDS_RESET) != 0
                || self.queues.iter().any(|queue| !queue.config.ready))
        {
            return Err(Error::InvalidQueue);
        }
        self.status = value;
        Ok(())
    }
}

fn validate_access(offset: u64, width: u8, value: u64) -> Result<(), Error> {
    if width != 4 || !offset.is_multiple_of(4) || value > u64::from(u32::MAX) {
        Err(Error::InvalidAccess)
    } else {
        Ok(())
    }
}

fn set_half(address: &mut u64, high: bool, value: u32) {
    let shift = if high { 32 } else { 0 };
    *address = (*address & !(u64::from(u32::MAX) << shift)) | (u64::from(value) << shift);
}
