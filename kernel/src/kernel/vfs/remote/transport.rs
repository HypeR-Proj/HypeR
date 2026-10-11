// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ordered filesystem RPC. Shared bytes are never interpreted as Rust objects.
//! A timed-out or malformed transaction permanently retires this
//! generation: late server writes cannot become a subsequent request's reply.

use crate::kernel::{
    accounting::ResourceDomain,
    authority::Rights,
    capability::HandleValue,
    ipc::{ByteChannel, ByteChannelError, PreparedByteMessage},
    mm::user_space::{
        DomainAccount, KernelPageBackend, VmoObject, WritableMappingLease, WritableVmo,
    },
    object::{KernelObject, KernelRef, KernelService, SignalWaitOutcome, wait_one},
    process::Process,
};
use core::sync::atomic::{AtomicBool, Ordering};
use hyper::mm::FallibleArc;
use hyper_filesystem::{
    Timestamp,
    protocol::{self, DirectoryBatch, DirectoryCursor, Entry, Error, Operation, Request, Response},
};

pub(super) struct Health {
    channel: KernelRef<ByteChannel, KernelService>,
    failed: AtomicBool,
}
impl Health {
    pub(super) fn check(&self) -> Result<(), Error> {
        if self.failed.load(Ordering::Acquire) || self.channel.object().is_closed() {
            Err(Error::Closed)
        } else {
            Ok(())
        }
    }
    pub(super) fn fail(&self) {
        self.failed.store(true, Ordering::Release);
    }
}
pub(in crate::kernel::vfs) struct Transport {
    pub(super) health: FallibleArc<Health>,
    buffer: WritableVmo<KernelPageBackend, DomainAccount>,
    _mapping: WritableMappingLease<KernelPageBackend, DomainAccount>,
    domain: ResourceDomain,
    sequence: u64,
    read_only: bool,
}
impl Transport {
    pub(crate) fn prepare(
        process: &Process,
        channel: HandleValue,
        buffer: HandleValue,
        read_only: bool,
    ) -> Result<Self, super::Error> {
        let channel = process
            .resolve_handle::<ByteChannel>(
                channel,
                Rights::READ.union(Rights::WRITE).union(Rights::WAIT),
            )
            .map_err(|_| super::Error::InvalidInput)?;
        let object = process
            .resolve_handle::<VmoObject>(
                buffer,
                Rights::READ.union(Rights::WRITE).union(Rights::MAP),
            )
            .map_err(|_| super::Error::InvalidInput)?;
        let buffer = object
            .object()
            .writable_clone()
            .ok_or(super::Error::InvalidInput)?;
        if buffer.size() != protocol::DATA_BYTES as u64 {
            return Err(super::Error::InvalidInput);
        }
        let mapping = buffer
            .try_mapping_write_lease()
            .map_err(|_| super::Error::InvalidInput)?;
        buffer
            .populate(0, buffer.size())
            .map_err(|_| super::Error::Allocation)?;
        let channel = channel.into_operation_pin().into_filesystem_transport();
        let health = FallibleArc::try_new(Health {
            channel,
            failed: AtomicBool::new(false),
        })?;
        Ok(Self {
            health,
            buffer,
            _mapping: mapping,
            domain: process.resource_domain(),
            sequence: 0,
            read_only,
        })
    }
    fn call(
        &mut self,
        operation: Operation,
        path: &str,
        target: &str,
        offset: u64,
        length: u64,
        times: [Option<Timestamp>; 2],
    ) -> Result<Response, Error> {
        self.health.check()?;
        if self.read_only
            && matches!(
                operation,
                Operation::Write
                    | Operation::Create
                    | Operation::Remove
                    | Operation::Rename
                    | Operation::Resize
                    | Operation::SetMetadata
            )
        {
            return Err(Error::ReadOnly);
        }
        let result = self.exchange(Request {
            sequence: 0,
            operation,
            path,
            target,
            offset,
            length,
            accessed: times[0],
            modified: times[1],
            cursor: DirectoryCursor::START,
        });
        if result.as_ref().err().is_some_and(|error| error.terminal()) {
            self.health.fail();
        }
        result
    }
    fn exchange(&mut self, mut request: Request<'_>) -> Result<Response, Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::Closed)?;
        request.sequence = self.sequence;
        let operation = request.operation;
        let path = request.path;
        let target = request.target;
        let size = protocol::HEADER_BYTES
            .checked_add(path.len())
            .and_then(|n| n.checked_add(target.len()))
            .ok_or(Error::InvalidInput)?;
        let mut message =
            PreparedByteMessage::try_new(&self.domain, size).map_err(|_| Error::Allocation)?;
        request
            .encode(message.bytes_mut())
            .ok_or(Error::InvalidInput)?;
        // Object waits consume absolute nanoseconds, not hardware counter ticks.
        // Prepare a finite deadline before transferring the shared range.
        let deadline_nanoseconds = crate::kernel::time::monotonic_nanoseconds()
            .map_err(|_| Error::Closed)?
            .checked_add(60_000_000_000)
            .filter(|deadline| *deadline != hyper::abi::native::HYPER_NATIVE_DEADLINE_INFINITE)
            .ok_or(Error::Closed)?;
        let channel = self.health.channel.object();
        let reservation = channel.prepare_write(&message).map_err(|_| Error::Closed)?;
        reservation.publish(message);
        loop {
            match channel.peek() {
                Ok(info) => {
                    let claim = channel.claim(info).map_err(|_| Error::Closed)?;
                    let response = Response::decode(claim.bytes(), self.sequence, operation);
                    claim.commit().release();
                    let response = response.ok_or(Error::Corrupt)?;
                    response.result?;
                    return Ok(response);
                }
                Err(ByteChannelError::WouldBlock) => {}
                Err(_) => return Err(Error::Closed),
            }
            let source = channel.signal_source().ok_or(Error::Closed)?;
            match wait_one(
                source,
                &self.domain,
                ByteChannel::READABLE.union(ByteChannel::PEER_CLOSED).bits(),
                deadline_nanoseconds,
                // Publication transfers the shared range to the worker. An
                // app's stop request cannot retire a mount used by other apps.
                // Drain this bounded transaction; the user runner handles stop
                // after the syscall unwinds. Scheduler cancellation selects one
                // current wait ticket, not every subsequent rearm.
                || false,
            )
            .map_err(|_| Error::Closed)?
            {
                SignalWaitOutcome::Observed(_) | SignalWaitOutcome::Cancelled => {}
                SignalWaitOutcome::TimedOut => return Err(Error::Closed),
            }
        }
    }
    pub(super) fn stat_into(&mut self, path: &str, entry: &mut Entry) -> Result<(), Error> {
        *entry = self.call(Operation::Stat, path, "", 0, 0, [None; 2])?.entry;
        Ok(())
    }
    /// Directory checkpoints originate in a validated backend batch and stay in
    /// the kernel-owned stream. Native cookies never enter this request.
    pub(super) fn read_directory(
        &mut self,
        path: &str,
        cursor: DirectoryCursor,
        output: &mut [u8; protocol::DIRECTORY_BATCH_BYTES],
    ) -> Result<usize, Error> {
        self.health.check()?;
        let result = (|| {
            let response = self.exchange(Request {
                sequence: 0,
                operation: Operation::ReadDirectory,
                path,
                target: "",
                offset: 0,
                length: output.len() as u64,
                accessed: None,
                modified: None,
                cursor,
            })?;
            let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
            let destination = output.get_mut(..actual).ok_or(Error::Corrupt)?;
            self.buffer
                .read_exposed(0, destination)
                .map_err(|_| Error::Io)?;
            let batch = DirectoryBatch::decode(destination).ok_or(Error::Corrupt)?;
            if batch.start_cursor() != cursor {
                return Err(Error::Corrupt);
            }
            Ok(actual)
        })();
        if result.as_ref().err().is_some_and(|error| error.terminal()) {
            self.health.fail();
        }
        result
    }
    pub(super) fn read_link(&mut self, path: &str, output: &mut [u8]) -> Result<usize, Error> {
        if output.len() > protocol::PATH_BYTES {
            return Err(Error::InvalidInput);
        }
        let response = self.call(
            Operation::ReadLink,
            path,
            "",
            0,
            output.len() as u64,
            [None; 2],
        )?;
        let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
        if actual > output.len() {
            self.health.fail();
            return Err(Error::Corrupt);
        }
        self.buffer
            .read_exposed(0, &mut output[..actual])
            .map_err(|_| Error::Io)?;
        Ok(actual)
    }
    pub(super) fn read_at(
        &mut self,
        path: &str,
        offset: u64,
        output: &mut [u8],
    ) -> Result<usize, Error> {
        let mut done = 0;
        for chunk in output.chunks_mut(protocol::DATA_BYTES) {
            let response = self.call(
                Operation::Read,
                path,
                "",
                offset.checked_add(done as u64).ok_or(Error::InvalidInput)?,
                chunk.len() as u64,
                [None; 2],
            )?;
            let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
            if actual > chunk.len() {
                self.health.fail();
                return Err(Error::Corrupt);
            }
            self.buffer
                .read_exposed(0, &mut chunk[..actual])
                .map_err(|_| Error::Io)?;
            done += actual;
            if actual < chunk.len() {
                break;
            }
        }
        Ok(done)
    }
    pub(super) fn write_at(
        &mut self,
        path: &str,
        offset: u64,
        input: &[u8],
    ) -> Result<usize, Error> {
        let mut done = 0;
        for chunk in input.chunks(protocol::DATA_BYTES) {
            self.health.check()?;
            self.buffer.write_exposed(0, chunk).map_err(|_| Error::Io)?;
            let response = self.call(
                Operation::Write,
                path,
                "",
                offset.checked_add(done as u64).ok_or(Error::InvalidInput)?,
                chunk.len() as u64,
                [None; 2],
            )?;
            let actual = usize::try_from(response.result?).map_err(|_| Error::Corrupt)?;
            if actual > chunk.len() {
                self.health.fail();
                return Err(Error::Corrupt);
            }
            done += actual;
            if actual < chunk.len() {
                break;
            }
        }
        Ok(done)
    }
    pub(super) fn create(&mut self, path: &str, directory: bool) -> Result<(), Error> {
        self.call(
            Operation::Create,
            path,
            "",
            0,
            u64::from(directory),
            [None; 2],
        )
        .map(|_| ())
    }
    pub(super) fn remove(&mut self, path: &str) -> Result<(), Error> {
        self.call(Operation::Remove, path, "", 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn rename(&mut self, path: &str, target: &str) -> Result<(), Error> {
        self.call(Operation::Rename, path, target, 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn resize(&mut self, path: &str, length: u64) -> Result<(), Error> {
        self.call(Operation::Resize, path, "", 0, length, [None; 2])
            .map(|_| ())
    }
    pub(super) fn sync(&mut self) -> Result<(), Error> {
        self.call(Operation::Sync, "", "", 0, 0, [None; 2])
            .map(|_| ())
    }
    pub(super) fn set_times(
        &mut self,
        path: &str,
        accessed: Option<Timestamp>,
        modified: Option<Timestamp>,
    ) -> Result<(), Error> {
        self.call(Operation::SetMetadata, path, "", 0, 0, [accessed, modified])
            .map(|_| ())
    }
}
