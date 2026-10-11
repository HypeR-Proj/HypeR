// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::ops::Range;
use core::sync::atomic::{Ordering, fence};

use super::{Error, GuestMemory, QUEUE_MAX, ServiceError, Transfer};

#[derive(Clone, Copy, Default)]
pub(super) struct Configuration {
    pub size: u32,
    pub descriptor: u64,
    pub available: u64,
    pub used: u64,
    pub ready: bool,
}

impl Configuration {
    pub fn regions(self) -> [(u64, u64, u64); 3] {
        let count = u64::from(self.size);
        [
            (self.descriptor, 16 * count, 16),
            (self.available, 6 + 2 * count, 2),
            (self.used, 6 + 8 * count, 4),
        ]
    }

    pub fn validate(self, ram: Range<u64>) -> Result<(), Error> {
        if self.size == 0 || !self.size.is_power_of_two() || self.size > u32::from(QUEUE_MAX) {
            return Err(Error::InvalidQueue);
        }
        let regions = self.regions();
        for (index, (base, len, alignment)) in regions.iter().copied().enumerate() {
            if !base.is_multiple_of(alignment) || !contains(&ram, base, len) {
                return Err(Error::InvalidQueue);
            }
            if regions[..index]
                .iter()
                .any(|(old_base, old_len, _)| base < old_base + old_len && *old_base < base + len)
            {
                return Err(Error::InvalidQueue);
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct SplitQueue {
    pub config: Configuration,
    next_available: u16,
    next_used: u16,
    initialized: bool,
    transmit: Option<Chain>,
}

impl SplitQueue {
    pub fn transmit(
        &mut self,
        memory: &impl GuestMemory,
        output: &mut [u8],
        ram: Range<u64>,
        regions: &[(u64, u64); 6],
        interrupt: &mut u32,
    ) -> Result<Transfer, ServiceError> {
        let mut bytes = 0;
        let result = (|| {
            self.initialize(memory)?;
            for _ in 0..QUEUE_MAX {
                let Some(mut chain) = self.take_chain(memory, false, ram.clone(), regions)? else {
                    break;
                };
                while chain.cursor < chain.count && bytes < output.len() {
                    let descriptor = chain.descriptors[chain.cursor];
                    let length = (descriptor.length - chain.offset).min(output.len() - bytes);
                    if length != 0 {
                        memory.read(
                            descriptor.address + chain.offset as u64,
                            &mut output[bytes..bytes + length],
                        )?;
                    }
                    bytes += length;
                    chain.offset += length;
                    if chain.offset == descriptor.length {
                        chain.cursor += 1;
                        chain.offset = 0;
                    }
                }
                if chain.cursor != chain.count {
                    self.transmit = Some(chain);
                    break;
                }
                // TX descriptors are device-readable. No bytes were written to the
                // guest, so the split-ring used length is zero, not the output size.
                self.complete(memory, chain.head, 0, interrupt)?;
                if bytes == output.len() {
                    break;
                }
            }
            Ok(Transfer {
                bytes,
                more: self.transmit.is_some() || self.pending(memory)? != 0,
            })
        })();
        result.map_err(|error| ServiceError { error, bytes })
    }

    pub fn receive(
        &mut self,
        memory: &impl GuestMemory,
        input: &[u8],
        ram: Range<u64>,
        regions: &[(u64, u64); 6],
        interrupt: &mut u32,
    ) -> Result<Transfer, ServiceError> {
        let mut bytes = 0;
        let result = (|| {
            self.initialize(memory)?;
            for _ in 0..QUEUE_MAX {
                let Some(chain) = self.take_chain(memory, true, ram.clone(), regions)? else {
                    break;
                };
                let mut written = 0;
                for descriptor in &chain.descriptors[..chain.count] {
                    let remaining = (input.len() - bytes).min(u32::MAX as usize - written);
                    let length = descriptor.length.min(remaining);
                    if length != 0 {
                        memory.write(descriptor.address, &input[bytes..bytes + length])?;
                    }
                    bytes += length;
                    written += length;
                    if bytes == input.len() || written == u32::MAX as usize {
                        break;
                    }
                }
                self.complete(memory, chain.head, written as u32, interrupt)?;
                if bytes == input.len() {
                    break;
                }
            }
            Ok(Transfer {
                bytes,
                more: self.pending(memory)? != 0,
            })
        })();
        result.map_err(|error| ServiceError { error, bytes })
    }

    fn initialize(&mut self, memory: &impl GuestMemory) -> Result<(), Error> {
        if !self.config.ready {
            return Err(Error::InvalidQueue);
        }
        if !self.initialized {
            // Kicks stay enabled throughout service. This avoids a notification
            // suppression/rearm race; interrupt suppression is independently
            // honored through the guest's available-ring flags below.
            memory.store_u16(self.config.used, 0)?;
            memory.store_u16(self.config.used + 2, 0)?;
            self.initialized = true;
        }
        Ok(())
    }

    fn pending(&self, memory: &impl GuestMemory) -> Result<u16, Error> {
        let available = memory.load_u16(self.config.available + 2)?;
        // The guest publishes descriptors and ring heads before avail.idx. The
        // scalar load and this acquire fence order all subsequent buffer reads.
        fence(Ordering::Acquire);
        if u32::from(available.wrapping_sub(self.next_used)) > self.config.size {
            return Err(Error::InvalidQueue);
        }
        Ok(available.wrapping_sub(self.next_available))
    }

    fn take_chain(
        &mut self,
        memory: &impl GuestMemory,
        writable: bool,
        ram: Range<u64>,
        regions: &[(u64, u64); 6],
    ) -> Result<Option<Chain>, Error> {
        if let Some(chain) = self.transmit.take() {
            return Ok(Some(chain));
        }
        if self.pending(memory)? == 0 {
            return Ok(None);
        }
        let slot = u64::from(self.next_available) & (u64::from(self.config.size) - 1);
        let head = memory.load_u16(self.config.available + 4 + 2 * slot)?;
        let chain = Chain::read(memory, self.config, head, writable, ram, regions)?;
        self.next_available = self.next_available.wrapping_add(1);
        Ok(Some(chain))
    }

    fn complete(
        &mut self,
        memory: &impl GuestMemory,
        head: u16,
        length: u32,
        interrupt: &mut u32,
    ) -> Result<(), Error> {
        let slot = u64::from(self.next_used) & (u64::from(self.config.size) - 1);
        let mut entry = [0u8; 8];
        entry[..4].copy_from_slice(&u32::from(head).to_le_bytes());
        entry[4..].copy_from_slice(&length.to_le_bytes());
        memory.write(self.config.used + 4 + 8 * slot, &entry)?;
        let next = self.next_used.wrapping_add(1);
        // Payload and used elements precede the single-copy-atomic used.idx
        // publication. No descriptor is accessed again after its completion.
        fence(Ordering::Release);
        memory.store_u16(self.config.used + 2, next)?;
        self.next_used = next;
        // Store->load ordering is required here: otherwise the driver can miss
        // used.idx while we observe its old interrupt-suppression flag.
        fence(Ordering::SeqCst);
        if memory.load_u16(self.config.available)? & 1 == 0 {
            *interrupt |= 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Descriptor {
    address: u64,
    length: usize,
}

struct Chain {
    head: u16,
    descriptors: [Descriptor; QUEUE_MAX as usize],
    count: usize,
    cursor: usize,
    offset: usize,
}

impl Chain {
    fn read(
        memory: &impl GuestMemory,
        queue: Configuration,
        head: u16,
        writable: bool,
        ram: Range<u64>,
        regions: &[(u64, u64); 6],
    ) -> Result<Self, Error> {
        let mut chain = Self {
            head,
            descriptors: [Descriptor::default(); QUEUE_MAX as usize],
            count: 0,
            cursor: 0,
            offset: 0,
        };
        let mut visited = [false; QUEUE_MAX as usize];
        let mut index = head;
        for _ in 0..queue.size {
            if u32::from(index) >= queue.size || visited[index as usize] {
                return Err(Error::InvalidDescriptor);
            }
            visited[index as usize] = true;
            let mut bytes = [0u8; 16];
            memory.read(queue.descriptor + u64::from(index) * 16, &mut bytes)?;
            let address = u64::from_le_bytes(bytes[..8].try_into().map_err(|_| Error::Memory)?);
            let length = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| Error::Memory)?);
            let flags = u16::from_le_bytes([bytes[12], bytes[13]]);
            let next = u16::from_le_bytes([bytes[14], bytes[15]]);
            if flags & !3 != 0 || (flags & 2 != 0) != writable {
                return Err(Error::InvalidDescriptor);
            }
            if !contains(&ram, address, u64::from(length))
                || regions.iter().any(|(start, end)| {
                    length != 0 && address < *end && *start < address + u64::from(length)
                })
            {
                return Err(Error::InvalidDescriptor);
            }
            chain.descriptors[chain.count] = Descriptor {
                address,
                length: length as usize,
            };
            chain.count += 1;
            if flags & 1 == 0 {
                return Ok(chain);
            }
            index = next;
        }
        Err(Error::InvalidDescriptor)
    }
}

fn contains(ram: &Range<u64>, base: u64, length: u64) -> bool {
    base >= ram.start && base.checked_add(length).is_some_and(|end| end <= ram.end)
}
