// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Independently scheduled console devices. Register requests, queue state and
//! resets have one owner; guest-memory access never borrows a guest pointer.

use super::readiness::{Readiness, Wake};
use super::{Input, memory::Memory, uart::Model};
use crate::serial::{ns16550::Ns16550, sbsa_uart::SbsaUart};
use crate::virtio_console;
use hyper_os::handle::{
    ByteChannelObject, GuestMemoryObject, OwnedHandle, VirtualCpuObject, VirtualMachineObject,
};
use hyper_os::wait::{ObjectSignals, WaitItem};
use hyper_os::{Error, Result, Status, channel, vm};
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

const CAPACITY: usize = 64 * 1024;
const BATCH: usize = super::MESSAGE_BYTES;
const REQUEST_SCANS: usize = 16;
// Bound interactive latency while amortizing transport and terminal wakeups.
const OUTPUT_FLUSH_NS: u64 = 2_000_000;
// Cookies are scoped to a VM and disjoint from broker-assigned small IDs.
const DEVICE: u64 = 0x5541_5254;
const VIRTIO_DEVICE: u64 = 0x434f_4e53;

/// Runtime end of one bounded duplex guest console byte stream. Closing it stops the
/// worker and requests VM shutdown; a failed console must not strand a vCPU.
pub struct Port {
    channel: OwnedHandle<ByteChannelObject>,
    thread: JoinHandle<Result<()>>,
}
impl Port {
    pub fn start(
        machine: Arc<OwnedHandle<VirtualMachineObject>>,
        cpus: Vec<Arc<OwnedHandle<VirtualCpuObject>>>,
        profile: vm::PlatformProfile,
        memory: OwnedHandle<GuestMemoryObject>,
        ram_base: u64,
        ram_size: u64,
        input: Input,
    ) -> Result<Self> {
        let (model, base, length, interrupt) = match profile {
            vm::PlatformProfile::Aarch64Reference => {
                (Model::Sbsa(SbsaUart::new()), 0x0900_0000, 0x1000, 33)
            }
            vm::PlatformProfile::Riscv64Reference => (
                Model::Ns16550(Ns16550::with_clock(1_000_000_000, 3_686_400)),
                0x1000_0000,
                0x1000,
                10,
            ),
        };
        let device = NonZeroU64::new(DEVICE).ok_or(Error::InvalidResponse)?;
        let route = vm::DeviceDoorbell::register(machine.as_handle_ref(), base, length, device)?;
        if profile == vm::PlatformProfile::Riscv64Reference {
            route.bind_firmware_console(machine.as_handle_ref())?;
        }
        let (virtio_base, virtio_interrupt) = match profile {
            vm::PlatformProfile::Aarch64Reference => (0x0a00_2000, 50),
            vm::PlatformProfile::Riscv64Reference => (0x1000_1000, 1),
        };
        let virtio =
            virtio_console::Device::new(ram_base, ram_size).map_err(|_| Error::InvalidResponse)?;
        let virtio_route = vm::DeviceDoorbell::register(
            machine.as_handle_ref(),
            virtio_base,
            0x1000,
            NonZeroU64::new(VIRTIO_DEVICE).ok_or(Error::InvalidResponse)?,
        )?;
        let (client, server) = channel::create_pair()?;
        let worker = Worker {
            machine,
            cpus,
            route,
            channel: server,
            model,
            base,
            interrupt,
            asserted: false,
            virtio,
            virtio_route,
            virtio_base,
            virtio_interrupt,
            virtio_asserted: false,
            virtio_pending: false,
            memory: Memory {
                grant: memory,
                base: ram_base,
            },
            input_transport: input,
            input: VecDeque::with_capacity(BATCH),
            output: VecDeque::with_capacity(CAPACITY),
        };
        let thread = thread::Builder::new()
            .name("vm-console".into())
            .stack_size(64 * 1024)
            .spawn(move || worker.run())
            .map_err(|_| Error::Status(Status::NO_MEMORY))?;
        Ok(Self {
            channel: client,
            thread,
        })
    }
    pub fn as_handle_ref(&self) -> hyper_os::HandleRef<'_, ByteChannelObject> {
        self.channel.as_handle_ref()
    }
    /// Receives one complete output message. Supply at least `console::MESSAGE_BYTES` capacity.
    pub fn try_read(&self, bytes: &mut [u8]) -> Result<usize> {
        match self.channel.as_byte_channel().try_receive(bytes) {
            Err(Error::Status(Status::WOULD_BLOCK | Status::PEER_CLOSED)) => Ok(0),
            result => result,
        }
    }
    pub fn try_write(&self, bytes: &[u8]) -> Result<usize> {
        let count = bytes.len().min(BATCH);
        if count == 0 {
            return Ok(0);
        }
        self.channel.as_byte_channel().try_send(&bytes[..count])?;
        Ok(count)
    }
    pub fn wait_item(&self) -> WaitItem<'_> {
        WaitItem::new(
            self.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )
    }
    /// Call after requesting VM stop. Joins independently of guest progress;
    /// channel closure wakes backpressure/input waits without polling.
    pub fn join(self) -> Result<()> {
        let Self { channel, thread } = self;
        drop(channel);
        thread.join().map_err(|_| Error::Status(Status::INTERNAL))?
    }
}

struct Worker {
    machine: Arc<OwnedHandle<VirtualMachineObject>>,
    cpus: Vec<Arc<OwnedHandle<VirtualCpuObject>>>,
    route: vm::DeviceDoorbell,
    channel: OwnedHandle<ByteChannelObject>,
    model: Model,
    base: u64,
    interrupt: u32,
    asserted: bool,
    virtio: virtio_console::Device,
    virtio_route: vm::DeviceDoorbell,
    virtio_base: u64,
    virtio_interrupt: u32,
    virtio_asserted: bool,
    virtio_pending: bool,
    memory: Memory,
    input_transport: Input,
    input: VecDeque<u8>,
    output: VecDeque<u8>,
}
impl Worker {
    fn now() -> Result<u64> {
        Ok(hyper_os::time::monotonic_now()?.as_nanoseconds())
    }
    fn input(&mut self, now: u64) {
        self.model.advance(now);
        while self.input_transport == Input::Uart && self.model.can_receive() {
            let Some(byte) = self.input.pop_front() else {
                break;
            };
            self.model.receive(byte, now);
        }
    }
    fn collect_output(&mut self, now: u64, deadline: &mut Option<u64>) {
        let Model::Sbsa(uart) = &mut self.model else {
            return;
        };
        while self.output.len() < CAPACITY {
            let Some(byte) = uart.take_transmitted() else {
                break;
            };
            deadline.get_or_insert(now.saturating_add(OUTPUT_FLUSH_NS));
            self.output.push_back(byte);
        }
    }
    fn can_complete(&self, request: &vm::DeviceRequest) -> bool {
        match (&self.model, request) {
            (Model::Sbsa(uart), vm::DeviceRequest::Mmio(mmio)) => {
                // A full TX path must not prevent the guest from reading
                // status, consuming RX data, or acknowledging an interrupt.
                // Keep only an unserviceable DR write in its durable slot.
                mmio.address != self.base
                    || !matches!(mmio.operation, vm::MmioOperation::Write(_))
                    || uart.can_transmit()
            }
            _ => self.output.len() < CAPACITY,
        }
    }
    fn interrupt(&mut self) -> Result<()> {
        let asserted = self.model.asserted();
        if asserted != self.asserted {
            vm::set_device_interrupt(self.machine.as_handle_ref(), self.interrupt, asserted)?;
            self.asserted = asserted;
        }
        let asserted = self.virtio.irq_asserted();
        if asserted != self.virtio_asserted {
            vm::set_device_interrupt(
                self.machine.as_handle_ref(),
                self.virtio_interrupt,
                asserted,
            )?;
            self.virtio_asserted = asserted;
        }
        Ok(())
    }

    fn service_queues(&mut self, now: u64, deadline: &mut Option<u64>) {
        if self.input_transport == Input::Virtio && !self.input.is_empty() {
            match self
                .virtio
                .receive(&self.memory, self.input.make_contiguous())
            {
                Ok(transfer) => {
                    self.input.drain(..transfer.bytes);
                }
                Err(error) => {
                    self.input.clear();
                    eprintln!(
                        "HypeR virtio console: receive queue fault: {:?}",
                        error.error
                    );
                }
            }
        }
        let mut bytes = [0; BATCH];
        self.virtio_pending = false;
        // A single notification can cover many buffers. Bound this pass, but
        // keep servicing durable queue work without requiring another kick.
        for _ in 0..16 {
            let capacity = (CAPACITY - self.output.len()).min(BATCH);
            if capacity == 0 {
                break;
            }
            match self.virtio.transmit(&self.memory, &mut bytes[..capacity]) {
                Ok(transfer) => {
                    self.virtio_pending = transfer.more;
                    if transfer.bytes != 0 {
                        deadline.get_or_insert(now.saturating_add(OUTPUT_FLUSH_NS));
                        self.output.extend(&bytes[..transfer.bytes]);
                    }
                    if !transfer.more {
                        break;
                    }
                }
                Err(error) => {
                    // Earlier buffers may already be acknowledged. Preserve
                    // that copied prefix even if a later descriptor is invalid.
                    if error.bytes != 0 {
                        deadline.get_or_insert(now.saturating_add(OUTPUT_FLUSH_NS));
                        self.output.extend(&bytes[..error.bytes]);
                    }
                    self.virtio_pending = false;
                    eprintln!(
                        "HypeR virtio console: transmit queue fault: {:?}",
                        error.error
                    );
                    break;
                }
            }
        }
    }
    fn run(mut self) -> Result<()> {
        let result = match self.service() {
            // Lifecycle admission closes before the termination signal publishes.
            Err(Error::Status(Status::BAD_STATE)) => Ok(()),
            result => result,
        };
        // Stop also cancels outstanding detached MMIO. The worker owns VM/vCPU
        // capabilities until it leaves, independently of caller teardown.
        let _ = vm::request_stop(self.machine.as_handle_ref());
        if let Err(error) = &result {
            eprintln!("HypeR guest console: service failed: {error:?}");
        }
        result
    }
    fn service(&mut self) -> Result<()> {
        let mut bytes = [0; BATCH];
        let mut readiness = Readiness::new(
            self.channel.as_handle_ref(),
            self.machine.as_handle_ref(),
            &self.route,
            &self.virtio_route,
        )?;
        let mut input_ready = true;
        let mut output_ready = true;
        let mut output_deadline = None;
        loop {
            self.route.prepare_scan()?;
            self.virtio_route.prepare_scan()?;
            if self.input.is_empty() && input_ready {
                match self.channel.as_byte_channel().try_receive(&mut bytes) {
                    Ok(count) => self.input.extend(&bytes[..count]),
                    Err(Error::Status(Status::WOULD_BLOCK)) => input_ready = false,
                    Err(Error::Status(Status::PEER_CLOSED)) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
            let now = self.drain_requests(&mut output_deadline)?;
            self.service_queues(now, &mut output_deadline);
            self.interrupt()?;
            let blocked = self.output.len() == CAPACITY;
            let flush = self.output.len() >= BATCH
                || output_deadline.is_some_and(|deadline| now >= deadline);
            while flush && !self.output.is_empty() && output_ready {
                let count = self.output.len().min(BATCH);
                for (out, byte) in bytes[..count].iter_mut().zip(self.output.iter()) {
                    *out = *byte;
                }
                match self.channel.as_byte_channel().try_send(&bytes[..count]) {
                    Ok(()) => {
                        self.output.drain(..count);
                    }
                    Err(Error::Status(Status::WOULD_BLOCK)) => {
                        output_ready = false;
                        break;
                    }
                    Err(Error::Status(Status::PEER_CLOSED)) => return Ok(()),
                    Err(error) => return Err(error),
                }
            }
            if self.output.is_empty() {
                output_deadline = None;
            }
            // Even a continuously busy TX queue must consume input and closure
            // notifications. Poll readiness between bounded service passes.
            let retry = (blocked || self.virtio_pending) && self.output.len() < CAPACITY;
            match readiness.wait(
                matches!(self.model, Model::Sbsa(_)) || self.output.len() < CAPACITY,
                self.input.is_empty(),
                !self.output.is_empty() && !output_ready,
                self.model.deadline().min(if retry {
                    now
                } else if output_ready {
                    output_deadline.unwrap_or(u64::MAX)
                } else {
                    u64::MAX
                }),
            )? {
                Wake::Input => input_ready = true,
                Wake::Output => output_ready = true,
                Wake::Closed => return Ok(()),
                Wake::Device | Wake::Timeout => {}
            }
        }
    }
    fn drain_requests(&mut self, output_deadline: &mut Option<u64>) -> Result<u64> {
        let mut now = Self::now()?;
        // Drain requests already available after a completion, without a
        // wait/rearm round trip per register. Stop at the first empty scan;
        // a fixed budget and the output deadline keep other work live.
        for _ in 0..REQUEST_SCANS {
            self.input(now);
            self.collect_output(now, output_deadline);
            self.interrupt()?;
            let mut progress = false;
            for index in 0..self.cpus.len() {
                if let Some(request) = self
                    .virtio_route
                    .pending(self.cpus[index].as_handle_ref())?
                {
                    let vm::DeviceRequest::Mmio(mmio) = request else {
                        return Err(Error::InvalidResponse);
                    };
                    let offset = mmio
                        .address
                        .checked_sub(self.virtio_base)
                        .ok_or(Error::InvalidResponse)?;
                    let completion = match mmio.operation {
                        vm::MmioOperation::Read => vm::MmioCompletion::Read(
                            self.virtio
                                .read(offset, mmio.width as u8)
                                .map_err(|error| {
                                    eprintln!(
                                        "HypeR virtio console: MMIO read {offset:#x}/{}: {error:?}",
                                        mmio.width
                                    );
                                    Error::InvalidResponse
                                })?,
                        ),
                        vm::MmioOperation::Write(value) => {
                            self.virtio
                                .write(offset, mmio.width as u8, value)
                                .map_err(|error| {
                                    eprintln!("HypeR virtio console: MMIO write {offset:#x}/{}={value:#x}: {error:?}", mmio.width);
                                    Error::InvalidResponse
                                })?;
                            vm::MmioCompletion::Write
                        }
                    };
                    self.interrupt()?;
                    // Queue kicks complete before buffer processing. The guest
                    // need not wait for terminal output to finish this MMIO.
                    self.virtio_route.complete(
                        self.cpus[index].as_handle_ref(),
                        request,
                        completion,
                    )?;
                    progress = true;
                    continue;
                }
                let Some(request) = self.route.pending(self.cpus[index].as_handle_ref())? else {
                    continue;
                };
                if !self.can_complete(&request) {
                    continue;
                }
                progress = true;
                let (completion, transmitted) = match request {
                    vm::DeviceRequest::Mmio(mmio) => {
                        let offset = mmio
                            .address
                            .checked_sub(self.base)
                            .ok_or(Error::InvalidResponse)?;
                        self.model.access(offset, mmio, now)?
                    }
                    vm::DeviceRequest::FirmwareConsoleWrite { byte, .. } => {
                        (vm::MmioCompletion::Write, Some(byte))
                    }
                };
                if let Some(byte) = transmitted {
                    output_deadline.get_or_insert(now.saturating_add(OUTPUT_FLUSH_NS));
                    self.output.push_back(byte);
                }
                self.input(now);
                self.collect_output(now, output_deadline);
                self.interrupt()?;
                self.route
                    .complete(self.cpus[index].as_handle_ref(), request, completion)?;
            }
            now = Self::now()?;
            if !progress || output_deadline.is_some_and(|deadline| now >= deadline) {
                break;
            }
        }
        Ok(now)
    }
}
