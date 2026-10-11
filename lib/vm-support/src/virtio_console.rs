// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Single-port virtio console, with all device policy owned by its userspace worker.
//!
//! A worker serializes MMIO, queue service and reset. Guest memory is untrusted:
//! queue metadata is validated before use, descriptor chains are copied into
//! bounded local state, and no borrowed guest pointer survives a memory operation.

mod queue;
mod registers;

use queue::SplitQueue;

pub const QUEUE_MAX: u16 = 128;
pub const VERSION_1: u64 = 1 << 32;
const RX: usize = 0;
const TX: usize = 1;
const DRIVER_OK: u32 = 4;
const NEEDS_RESET: u32 = 64;
const FAILED: u32 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAccess,
    InvalidStatus,
    InvalidQueue,
    InvalidDescriptor,
    Memory,
}

/// A terminal queue fault can follow successful work in the same bounded pass.
/// The copied prefix remains valid and must not disappear with the later error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceError {
    pub error: Error,
    pub bytes: usize,
}

/// Checked copies into the guest's retained RAM allocation.
///
/// Implementations must tolerate concurrent guest writes without exposing Rust
/// references to that memory. Scalar operations use aligned, single-copy-atomic
/// little-endian accesses; copying an index as two independent bytes is unsound
/// for the split-ring publication protocol. The model supplies the inter-access
/// acquire/release fences. These are coherent CPU accesses, not physical DMA.
pub trait GuestMemory {
    fn read(&self, address: u64, bytes: &mut [u8]) -> Result<(), Error>;
    fn write(&self, address: u64, bytes: &[u8]) -> Result<(), Error>;
    fn load_u16(&self, address: u64) -> Result<u16, Error>;
    fn store_u16(&self, address: u64, value: u16) -> Result<(), Error>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Transfer {
    /// Bytes copied to the caller's output, or consumed from its input.
    pub bytes: usize,
    /// The queue still has work. This is a scheduling hint, not a notification
    /// substitute: a guest may publish more buffers immediately after this check.
    pub more: bool,
}

/// An exclusively owned device. The caller must publish `irq_asserted()` to its
/// level interrupt after every MMIO operation and queue-service call, including
/// failures. A failed queue is frozen until the guest resets the device.
pub struct Device {
    memory_base: u64,
    memory_end: u64,
    queues: [SplitQueue; 2],
    device_selector: u32,
    driver_selector: u32,
    queue_selector: u32,
    features: u64,
    status: u32,
    interrupt_status: u32,
}

impl Device {
    pub fn new(memory_base: u64, memory_size: u64) -> Result<Self, Error> {
        let memory_end = memory_base
            .checked_add(memory_size)
            .filter(|end| *end > memory_base)
            .ok_or(Error::Memory)?;
        Ok(Self {
            memory_base,
            memory_end,
            queues: [SplitQueue::default(), SplitQueue::default()],
            device_selector: 0,
            driver_selector: 0,
            queue_selector: 0,
            features: 0,
            status: 0,
            interrupt_status: 0,
        })
    }

    pub fn irq_asserted(&self) -> bool {
        self.interrupt_status != 0
    }

    pub fn active(&self) -> bool {
        self.status & (DRIVER_OK | NEEDS_RESET | FAILED) == DRIVER_OK
    }

    /// Copies guest output into a caller-owned bounded buffer. A large chain is
    /// retained across calls and acknowledged only after its final byte is read.
    /// The caller must retain these bytes until its output transport accepts them,
    /// including the successful prefix reported by a later [`ServiceError`].
    pub fn transmit(
        &mut self,
        memory: &impl GuestMemory,
        output: &mut [u8],
    ) -> Result<Transfer, ServiceError> {
        if !self.active() || output.is_empty() {
            return Ok(Transfer::default());
        }
        let regions = self.ring_regions();
        let result = self.queues[TX].transmit(
            memory,
            output,
            self.memory_base..self.memory_end,
            &regions,
            &mut self.interrupt_status,
        );
        self.finish_service(result)
    }

    /// Delivers available input without waiting for guest receive buffers. The
    /// unconsumed suffix remains with the caller and is retried after a RX kick.
    /// A fault reports the copied prefix; the caller must not replay that prefix
    /// if it elects to retain the remaining input across a device reset.
    pub fn receive(
        &mut self,
        memory: &impl GuestMemory,
        input: &[u8],
    ) -> Result<Transfer, ServiceError> {
        if !self.active() || input.is_empty() {
            return Ok(Transfer::default());
        }
        let regions = self.ring_regions();
        let result = self.queues[RX].receive(
            memory,
            input,
            self.memory_base..self.memory_end,
            &regions,
            &mut self.interrupt_status,
        );
        self.finish_service(result)
    }

    fn finish_service(
        &mut self,
        result: Result<Transfer, ServiceError>,
    ) -> Result<Transfer, ServiceError> {
        if result.is_err() {
            self.status |= NEEDS_RESET;
            self.interrupt_status |= 2;
        }
        result
    }

    fn ring_regions(&self) -> [(u64, u64); 6] {
        let mut regions = [(0, 0); 6];
        for (index, queue) in self.queues.iter().enumerate() {
            if queue.config.ready {
                for (part, (base, length, _)) in queue.config.regions().into_iter().enumerate() {
                    regions[index * 3 + part] = (base, base + length);
                }
            }
        }
        regions
    }
}

#[cfg(test)]
#[path = "../tests/virtio_console.rs"]
mod tests;
