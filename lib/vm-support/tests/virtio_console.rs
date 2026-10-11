// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::cell::{Cell, RefCell};

use super::*;

type TestResult = Result<(), TestError>;

#[derive(Debug)]
struct TestError {
    error: String,
}

impl From<Error> for TestError {
    fn from(error: Error) -> Self {
        Self {
            error: format!("{error:?}"),
        }
    }
}

impl From<ServiceError> for TestError {
    fn from(error: ServiceError) -> Self {
        Self {
            error: format!("{error:?}"),
        }
    }
}

impl core::fmt::Display for TestError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.error)
    }
}
const BASE: u64 = 0x10000;
const SIZE: usize = 0x10000;
const DATA: u64 = BASE + 0x8000;
const COUNT: u16 = 8;

struct Memory {
    bytes: RefCell<Vec<u8>>,
    fail_write: Cell<bool>,
}

impl Memory {
    fn new() -> Self {
        Self {
            bytes: RefCell::new(vec![0; SIZE]),
            fail_write: Cell::new(false),
        }
    }

    fn index(address: u64, len: usize) -> Result<core::ops::Range<usize>, Error> {
        let start = usize::try_from(address.checked_sub(BASE).ok_or(Error::Memory)?)
            .map_err(|_| Error::Memory)?;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= SIZE)
            .ok_or(Error::Memory)?;
        Ok(start..end)
    }

    fn descriptor(
        &self,
        queue: usize,
        index: u16,
        address: u64,
        len: u32,
        flags: u16,
        next: u16,
    ) -> Result<(), Error> {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&address.to_le_bytes());
        bytes[8..12].copy_from_slice(&len.to_le_bytes());
        bytes[12..14].copy_from_slice(&flags.to_le_bytes());
        bytes[14..].copy_from_slice(&next.to_le_bytes());
        self.write(queue_base(queue) + 16 * u64::from(index), &bytes)
    }

    fn submit(&self, queue: usize, head: u16) -> Result<(), Error> {
        let avail = queue_base(queue) + 0x100;
        let index = self.load_u16(avail + 2)?;
        self.store_u16(avail + 4 + 2 * u64::from(index % COUNT), head)?;
        self.store_u16(avail + 2, index.wrapping_add(1))
    }

    fn used(&self, queue: usize) -> Result<u16, Error> {
        self.load_u16(queue_base(queue) + 0x202)
    }

    fn used_length(&self, queue: usize, index: u16) -> Result<u32, Error> {
        let mut bytes = [0u8; 4];
        self.read(
            queue_base(queue) + 0x208 + 8 * u64::from(index % COUNT),
            &mut bytes,
        )?;
        Ok(u32::from_le_bytes(bytes))
    }
}

impl GuestMemory for Memory {
    fn read(&self, address: u64, bytes: &mut [u8]) -> Result<(), Error> {
        bytes.copy_from_slice(&self.bytes.borrow()[Self::index(address, bytes.len())?]);
        Ok(())
    }

    fn write(&self, address: u64, bytes: &[u8]) -> Result<(), Error> {
        if self.fail_write.replace(false) {
            return Err(Error::Memory);
        }
        self.bytes.borrow_mut()[Self::index(address, bytes.len())?].copy_from_slice(bytes);
        Ok(())
    }

    fn load_u16(&self, address: u64) -> Result<u16, Error> {
        if !address.is_multiple_of(2) {
            return Err(Error::Memory);
        }
        let mut bytes = [0u8; 2];
        self.read(address, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn store_u16(&self, address: u64, value: u16) -> Result<(), Error> {
        if !address.is_multiple_of(2) {
            return Err(Error::Memory);
        }
        self.write(address, &value.to_le_bytes())
    }
}

fn queue_base(queue: usize) -> u64 {
    BASE + queue as u64 * 0x1000
}

fn negotiate(device: &mut Device) -> Result<(), Error> {
    device.write(0x70, 4, 1)?;
    device.write(0x70, 4, 3)?;
    device.write(0x24, 4, 1)?;
    device.write(0x20, 4, 1)?;
    device.write(0x70, 4, 11)
}

fn configure(device: &mut Device, queue: usize) -> Result<(), Error> {
    let base = queue_base(queue);
    device.write(0x30, 4, queue as u64)?;
    device.write(0x38, 4, u64::from(COUNT))?;
    device.write(0x80, 4, base)?;
    device.write(0x90, 4, base + 0x100)?;
    device.write(0xa0, 4, base + 0x200)?;
    device.write(0x44, 4, 1)
}

fn active() -> Result<(Device, Memory), Error> {
    let mut device = Device::new(BASE, SIZE as u64)?;
    negotiate(&mut device)?;
    configure(&mut device, RX)?;
    configure(&mut device, TX)?;
    device.write(0x70, 4, 15)?;
    Ok((device, Memory::new()))
}

#[test]
fn modern_single_port_transport_advertises_only_implemented_features() -> TestResult {
    let mut device = Device::new(BASE, SIZE as u64)?;
    assert_eq!(device.read(0, 4)?, 0x74726976);
    assert_eq!(device.read(4, 4)?, 2);
    assert_eq!(device.read(8, 4)?, 3);
    assert_eq!(device.read(0x10, 4)?, 0);
    device.write(0x14, 4, 1)?;
    assert_eq!(device.read(0x10, 4)?, 1);
    device.write(0x14, 4, 2)?;
    assert_eq!(device.read(0x10, 4)?, 0);
    assert_eq!(device.read(0x100, 1)?, 0);
    assert_eq!(device.read(0x102, 2)?, 0);
    assert_eq!(device.read(0x108, 4)?, 0);
    assert_eq!(device.read(0x10c, 4), Err(Error::InvalidAccess));
    assert_eq!(device.read(0x60, 2), Err(Error::InvalidAccess));
    assert_eq!(device.write(0x50, 4, 2), Err(Error::InvalidQueue));
    device.write(0x30, 4, 2)?;
    assert_eq!(device.read(0x34, 4)?, 0);
    assert_eq!(device.read(0x44, 4)?, 0);
    assert_eq!(device.write(0x44, 4, 1), Err(Error::InvalidQueue));
    Ok(())
}

#[test]
fn bad_negotiation_and_overlapping_rings_never_start_queue_access() -> TestResult {
    let mut device = Device::new(BASE, SIZE as u64)?;
    device.write(0x70, 4, 3)?;
    device.write(0x20, 4, 1)?;
    device.write(0x70, 4, 11)?;
    assert_eq!(device.read(0x70, 4)?, 3);
    assert_eq!(device.write(0x70, 4, 15), Err(Error::InvalidStatus));
    device.write(0x70, 4, 0)?;
    negotiate(&mut device)?;
    configure(&mut device, RX)?;
    device.write(0x30, 4, 1)?;
    device.write(0x38, 4, u64::from(COUNT))?;
    device.write(0x80, 4, BASE)?;
    device.write(0x90, 4, BASE + 0x100)?;
    device.write(0xa0, 4, BASE + 0x200)?;
    assert_eq!(device.write(0x44, 4, 1), Err(Error::InvalidQueue));
    assert!(!device.active());
    assert_eq!(
        device.transmit(&Memory::new(), &mut [0; 8])?,
        Transfer::default()
    );
    Ok(())
}

#[test]
fn transmit_copies_chains_in_order_and_completes_with_zero_written_length() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"hello world!")?;
    memory.descriptor(TX, 0, DATA, 5, 1, 3)?;
    memory.descriptor(TX, 3, DATA + 5, 7, 0, 0)?;
    memory.submit(TX, 0)?;
    let mut output = [0u8; 32];
    let result = device.transmit(&memory, &mut output)?;
    assert_eq!(
        result,
        Transfer {
            bytes: 12,
            more: false
        }
    );
    assert_eq!(&output[..12], b"hello world!");
    assert_eq!(memory.used(TX)?, 1);
    assert_eq!(memory.used_length(TX, 0)?, 0);
    assert_eq!(device.read(0x60, 4)?, 1);
    device.write(0x64, 4, 1)?;
    assert!(!device.irq_asserted());
    Ok(())
}

#[test]
fn partial_transmit_retains_owned_descriptors_until_final_byte() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"abcdefgh")?;
    memory.descriptor(TX, 0, DATA, 4, 1, 1)?;
    memory.descriptor(TX, 1, DATA + 4, 4, 0, 0)?;
    memory.submit(TX, 0)?;
    let mut output = [0u8; 3];
    assert_eq!(device.transmit(&memory, &mut output)?.bytes, 3);
    assert_eq!(&output, b"abc");
    assert_eq!(memory.used(TX)?, 0);
    // A malicious guest mutation cannot redirect the already accepted chain.
    memory.descriptor(TX, 1, u64::MAX, u32::MAX, 4, 99)?;
    assert_eq!(device.transmit(&memory, &mut output)?.bytes, 3);
    assert_eq!(&output, b"def");
    assert_eq!(memory.used(TX)?, 0);
    assert_eq!(
        device.transmit(&memory, &mut output)?,
        Transfer {
            bytes: 2,
            more: false
        }
    );
    assert_eq!(&output[..2], b"gh");
    assert_eq!(memory.used(TX)?, 1);
    Ok(())
}

#[test]
fn receive_uses_multiple_buffers_and_reports_actual_bytes_written() -> TestResult {
    let (mut device, memory) = active()?;
    memory.descriptor(RX, 0, DATA, 3, 3, 1)?;
    memory.descriptor(RX, 1, DATA + 3, 2, 2, 0)?;
    memory.descriptor(RX, 2, DATA + 16, 16, 2, 0)?;
    memory.submit(RX, 0)?;
    memory.submit(RX, 2)?;
    assert_eq!(
        device.receive(&memory, b"hello there")?,
        Transfer {
            bytes: 11,
            more: false
        }
    );
    let mut first = [0u8; 5];
    let mut second = [0u8; 6];
    memory.read(DATA, &mut first)?;
    memory.read(DATA + 16, &mut second)?;
    assert_eq!(&first, b"hello");
    assert_eq!(&second, b" there");
    assert_eq!(memory.used_length(RX, 0)?, 5);
    assert_eq!(memory.used_length(RX, 1)?, 6);
    assert_eq!(device.receive(&memory, b"pending")?.bytes, 0);
    Ok(())
}

#[test]
fn interrupt_suppression_does_not_suppress_used_publication() -> TestResult {
    let (mut device, memory) = active()?;
    memory.store_u16(queue_base(TX) + 0x100, 1)?;
    memory.descriptor(TX, 0, DATA, 0, 0, 0)?;
    memory.submit(TX, 0)?;
    assert_eq!(device.transmit(&memory, &mut [0; 8])?.bytes, 0);
    assert_eq!(memory.used(TX)?, 1);
    assert!(!device.irq_asserted());
    memory.store_u16(queue_base(TX) + 0x100, 0)?;
    memory.submit(TX, 0)?;
    device.transmit(&memory, &mut [0; 8])?;
    assert_eq!(memory.used(TX)?, 2);
    assert!(device.irq_asserted());
    Ok(())
}

#[test]
fn descriptor_direction_indirect_loops_and_bounds_are_rejected() -> TestResult {
    for (address, length, flags, next) in [
        (DATA, 1, 2, 0), // Writable TX buffer.
        (DATA, 1, 4, 0), // Unnegotiated indirect descriptor.
        (DATA, 1, 8, 0), // Reserved flags.
        (DATA, 1, 1, 0), // Self loop.
        (DATA, 1, 1, COUNT),
        (BASE - 1, 1, 0, 0),
        (u64::MAX, 2, 0, 0),
        (BASE + SIZE as u64, 1, 0, 0),
        (queue_base(RX) + 0x200, 1, 0, 0), // Payload aliases another ring.
    ] {
        let (mut device, memory) = active()?;
        memory.descriptor(TX, 0, address, length, flags, next)?;
        memory.submit(TX, 0)?;
        assert_eq!(
            device.transmit(&memory, &mut [0; 8]),
            Err(ServiceError {
                error: Error::InvalidDescriptor,
                bytes: 0
            })
        );
        assert_eq!(memory.used(TX)?, 0);
        assert_eq!(device.read(0x70, 4)?, 15 | 64);
        assert_eq!(device.read(0x60, 4)?, 2);
        assert!(!device.active());
    }
    let (mut device, memory) = active()?;
    memory.descriptor(RX, 0, DATA, 8, 0, 0)?;
    memory.submit(RX, 0)?;
    assert_eq!(
        device.receive(&memory, b"input"),
        Err(ServiceError {
            error: Error::InvalidDescriptor,
            bytes: 0
        })
    );
    Ok(())
}

#[test]
fn oversized_available_index_is_a_device_fault_not_an_unbounded_walk() -> TestResult {
    let (mut device, memory) = active()?;
    memory.store_u16(queue_base(TX) + 0x102, COUNT + 1)?;
    assert_eq!(
        device.transmit(&memory, &mut [0; 8]),
        Err(ServiceError {
            error: Error::InvalidQueue,
            bytes: 0
        })
    );
    assert_eq!(memory.used(TX)?, 0);
    Ok(())
}

#[test]
fn reset_discards_partial_chain_and_all_interrupt_state() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"abcdefgh")?;
    memory.descriptor(TX, 0, DATA, 8, 0, 0)?;
    memory.submit(TX, 0)?;
    assert_eq!(device.transmit(&memory, &mut [0; 3])?.bytes, 3);
    device.write(0x70, 4, 0)?;
    assert_eq!(device.read(0x60, 4)?, 0);
    assert_eq!(device.read(0x44, 4)?, 0);
    assert!(!device.active());
    negotiate(&mut device)?;
    configure(&mut device, RX)?;
    configure(&mut device, TX)?;
    device.write(0x70, 4, 15)?;
    let fresh = Memory::new();
    assert_eq!(device.transmit(&fresh, &mut [0; 8])?.bytes, 0);
    assert_eq!(fresh.used(TX)?, 0);
    Ok(())
}

#[test]
fn stopped_queue_is_quiescent_until_full_device_reset() -> TestResult {
    let (mut device, memory) = active()?;
    device.write(0x30, 4, 1)?;
    device.write(0x44, 4, 0)?;
    assert_eq!(device.read(0x44, 4)?, 0);
    assert_eq!(device.read(0x70, 4)?, 15 | 64);
    assert_eq!(device.transmit(&memory, &mut [0; 8])?.bytes, 0);
    assert_eq!(device.write(0x44, 4, 1), Err(Error::InvalidStatus));
    device.write(0x70, 4, 0)?;
    device.write(0x30, 4, 0)?;
    device.write(0x44, 4, 0)?;
    Ok(())
}

#[test]
fn memory_failure_stops_all_queue_access_until_reset() -> TestResult {
    let (mut device, memory) = active()?;
    memory.fail_write.set(true);
    assert_eq!(
        device.transmit(&memory, &mut [0; 8]),
        Err(ServiceError {
            error: Error::Memory,
            bytes: 0
        })
    );
    assert!(!device.active());
    assert_eq!(device.read(0x60, 4)?, 2);
    assert_eq!(device.receive(&memory, b"input")?.bytes, 0);
    Ok(())
}

#[test]
fn split_ring_indices_wrap_without_stalling_or_replaying_output() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"x")?;
    memory.descriptor(TX, 0, DATA, 1, 0, 0)?;
    for index in 1..=65540u32 {
        memory.submit(TX, 0)?;
        let mut byte = [0u8; 1];
        assert_eq!(device.transmit(&memory, &mut byte)?.bytes, 1);
        assert_eq!(&byte, b"x");
        assert_eq!(memory.used(TX)?, index as u16);
    }
    Ok(())
}

#[test]
fn transmit_batches_heads_and_keeps_the_unfinished_suffix() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"first-second-third")?;
    for (head, offset, length) in [(0, 0, 6), (1, 6, 7), (2, 13, 5)] {
        memory.descriptor(TX, head, DATA + offset, length, 0, 0)?;
        memory.submit(TX, head)?;
    }
    let mut batch = [0; 15];
    assert_eq!(
        device.transmit(&memory, &mut batch)?,
        Transfer {
            bytes: 15,
            more: true
        }
    );
    assert_eq!(&batch, b"first-second-th");
    assert_eq!(memory.used(TX)?, 2);
    assert_eq!(
        device.transmit(&memory, &mut batch)?,
        Transfer {
            bytes: 3,
            more: false
        }
    );
    assert_eq!(&batch[..3], b"ird");
    assert_eq!(memory.used(TX)?, 3);
    Ok(())
}

#[test]
fn queue_metadata_includes_trailing_fields_and_cannot_overlap() -> TestResult {
    for (descriptor, available, used) in [
        (BASE + 1, BASE + 0x100, BASE + 0x200),
        (BASE, BASE + 0x101, BASE + 0x200),
        (BASE, BASE + 0x100, BASE + 0x202),
        (BASE, BASE + 16, BASE + 0x200),
        (BASE, BASE + 0x100, BASE + 0x104),
        (
            BASE,
            BASE + 0x100,
            BASE + SIZE as u64 - (4 + 8 * u64::from(COUNT)),
        ),
        (u64::MAX - 15, BASE + 0x100, BASE + 0x200),
    ] {
        let mut device = Device::new(BASE, SIZE as u64)?;
        negotiate(&mut device)?;
        device.write(0x38, 4, u64::from(COUNT))?;
        for (offset, address) in [(0x80, descriptor), (0x90, available), (0xa0, used)] {
            device.write(offset, 4, address & u64::from(u32::MAX))?;
            device.write(offset + 4, 4, address >> 32)?;
        }
        assert_eq!(device.write(0x44, 4, 1), Err(Error::InvalidQueue));
        assert_eq!(device.read(0x44, 4)?, 0);
    }
    Ok(())
}

#[test]
fn receive_backpressure_preserves_the_callers_unconsumed_suffix() -> TestResult {
    let (mut device, memory) = active()?;
    assert_eq!(device.receive(&memory, b"hello")?, Transfer::default());
    memory.descriptor(RX, 0, DATA, 2, 2, 0)?;
    memory.submit(RX, 0)?;
    assert_eq!(
        device.receive(&memory, b"hello")?,
        Transfer {
            bytes: 2,
            more: false
        }
    );
    assert_eq!(memory.used_length(RX, 0)?, 2);
    memory.descriptor(RX, 0, DATA + 2, 8, 2, 0)?;
    memory.submit(RX, 0)?;
    assert_eq!(device.receive(&memory, b"llo")?.bytes, 3);
    let mut bytes = [0; 5];
    memory.read(DATA, &mut bytes)?;
    assert_eq!(&bytes, b"hello");
    Ok(())
}

#[test]
fn transmit_fault_preserves_output_already_acknowledged_in_the_same_pass() -> TestResult {
    let (mut device, memory) = active()?;
    memory.write(DATA, b"valid")?;
    memory.descriptor(TX, 0, DATA, 5, 0, 0)?;
    memory.descriptor(TX, 1, DATA, 5, 2, 0)?; // A later TX buffer is writable.
    memory.submit(TX, 0)?;
    memory.submit(TX, 1)?;
    let mut output = [0; 16];
    assert_eq!(
        device.transmit(&memory, &mut output),
        Err(ServiceError {
            error: Error::InvalidDescriptor,
            bytes: 5
        })
    );
    assert_eq!(&output[..5], b"valid");
    assert_eq!(memory.used(TX)?, 1);
    assert_eq!(device.read(0x70, 4)?, 15 | 64);
    assert_eq!(device.read(0x60, 4)?, 3);
    assert_eq!(device.transmit(&memory, &mut output)?, Transfer::default());
    Ok(())
}

#[test]
fn receive_fault_reports_the_prefix_already_delivered_to_the_guest() -> TestResult {
    let (mut device, memory) = active()?;
    memory.descriptor(RX, 0, DATA, 3, 2, 0)?;
    memory.descriptor(RX, 1, DATA + 16, 3, 0, 0)?; // A later RX buffer is read-only.
    memory.submit(RX, 0)?;
    memory.submit(RX, 1)?;
    assert_eq!(
        device.receive(&memory, b"abcdef"),
        Err(ServiceError {
            error: Error::InvalidDescriptor,
            bytes: 3
        })
    );
    let mut input = [0; 3];
    memory.read(DATA, &mut input)?;
    assert_eq!(&input, b"abc");
    assert_eq!(memory.used(RX)?, 1);
    assert_eq!(memory.used_length(RX, 0)?, 3);
    assert!(!device.active());
    Ok(())
}
