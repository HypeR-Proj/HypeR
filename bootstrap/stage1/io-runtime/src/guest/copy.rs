// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded image read-ahead. Exactly two buffers circulate between one reader
//! and its consumer; closing either channel cancels further work, and joining
//! the scoped reader retires every buffer before the source can disappear.

use std::io;
use std::sync::mpsc;

pub const BATCH_BYTES: usize = 512 * 1024;

#[derive(Debug)]
pub enum Error<E> {
    Read(io::Error),
    Write(E),
    Thread(io::Error),
    WorkerStopped,
    Allocation,
    InvalidRange,
}

impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(f, "image read: {error}"),
            Self::Write(error) => write!(f, "guest memory write: {error}"),
            Self::Thread(error) => write!(f, "image reader thread: {error}"),
            Self::WorkerStopped => f.write_str("image reader stopped"),
            Self::Allocation => f.write_str("image buffer allocation failed"),
            Self::InvalidRange => f.write_str("invalid image copy range"),
        }
    }
}

struct Batch {
    offset: u64,
    length: usize,
    bytes: Vec<u8>,
}

fn buffer(length: usize) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| ())?;
    bytes.resize(length, 0);
    Ok(bytes)
}

/// `write` receives offsets relative to the payload, never absolute file offsets.
/// A consumer failure cancels read-ahead and waits for an already-started read;
/// cancellation does not authorize reuse of a kernel-owned I/O buffer.
pub fn copy<R, W, E>(source_offset: u64, length: u64, read: R, mut write: W) -> Result<(), Error<E>>
where
    R: Fn(u64, &mut [u8]) -> io::Result<()> + Send,
    W: FnMut(u64, &[u8]) -> Result<(), E>,
{
    source_offset
        .checked_add(length)
        .ok_or(Error::InvalidRange)?;
    if length == 0 {
        return Ok(());
    }
    let mut first =
        buffer(length.min(BATCH_BYTES as u64) as usize).map_err(|_| Error::Allocation)?;
    if length <= BATCH_BYTES as u64 {
        read(source_offset, &mut first).map_err(Error::Read)?;
        write(0, &first).map_err(Error::Write)?;
        return Ok(());
    }
    let second = buffer(BATCH_BYTES).map_err(|_| Error::Allocation)?;
    let (free_tx, free_rx) = mpsc::sync_channel(2);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    free_tx.send(first).map_err(|_| Error::WorkerStopped)?;
    free_tx.send(second).map_err(|_| Error::WorkerStopped)?;
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("image-reader".into())
            .stack_size(128 * 1024)
            .spawn_scoped(scope, move || {
                let mut completed = 0;
                while completed < length {
                    let Ok(mut bytes) = free_rx.recv() else { break };
                    let offset = source_offset + completed;
                    // After the first batch, file requests start at a batch
                    // boundary. This avoids two extra sector reads on every
                    // unaligned FIT payload transfer.
                    let capacity = BATCH_BYTES - (offset % BATCH_BYTES as u64) as usize;
                    let count = (length - completed).min(capacity as u64) as usize;
                    let result = read(offset, &mut bytes[..count]);
                    if let Err(error) = result {
                        let _ = ready_tx.send(Err(error));
                        break;
                    }
                    if ready_tx
                        .send(Ok(Batch {
                            offset: completed,
                            length: count,
                            bytes,
                        }))
                        .is_err()
                    {
                        break;
                    }
                    completed += count as u64;
                }
            })
            .map_err(Error::Thread)?;
        let result = (|| {
            let mut completed = 0;
            while completed < length {
                let batch = ready_rx
                    .recv()
                    .map_err(|_| Error::WorkerStopped)?
                    .map_err(Error::Read)?;
                if batch.offset != completed
                    || batch.length == 0
                    || batch.length as u64 > length - completed
                {
                    return Err(Error::InvalidRange);
                }
                write(completed, &batch.bytes[..batch.length]).map_err(Error::Write)?;
                completed += batch.length as u64;
                // A finished reader may already have dropped its receiver.
                // The final ready batch or its error still belongs to ready_rx.
                let _ = free_tx.send(batch.bytes);
            }
            Ok(())
        })();
        drop(ready_rx);
        drop(free_tx);
        let reader = worker.join();
        result?;
        reader.map_err(|_| Error::WorkerStopped)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "../../tests/image_io.rs"]
mod tests;
