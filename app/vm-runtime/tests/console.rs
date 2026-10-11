// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{BATCH, RETAIN_BYTES, can_collect_output, collect_output, retain};
use hyper_os::Result;
use std::collections::VecDeque;

struct Output {
    bytes: Vec<u8>,
    position: usize,
    reads: usize,
}

impl Output {
    fn read(&mut self, output: &mut [u8]) -> Result<usize> {
        self.reads += 1;
        let count = output.len().min(self.bytes.len() - self.position);
        output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

#[test]
fn disconnected_client_keeps_bounded_latest_output() {
    let mut queue = VecDeque::new();
    retain(&mut queue, &vec![b'a'; RETAIN_BYTES]);
    retain(&mut queue, b"new");
    assert_eq!(queue.len(), RETAIN_BYTES);
    assert_eq!(
        queue.iter().rev().take(3).copied().collect::<Vec<_>>(),
        b"wen"
    );
}

#[test]
fn attached_slow_reader_preserves_every_byte_across_repeated_backpressure() -> Result<()> {
    let mut source = Output {
        bytes: (0..RETAIN_BYTES * 3 + 123)
            .map(|index| (index % 251) as u8)
            .collect(),
        position: 0,
        reads: 0,
    };
    let mut retained = VecDeque::new();
    let mut scratch = [0; BATCH];
    // Initially the client does not read at all. Collection must stop before
    // receiving a message whose tail would evict already accepted output.
    while can_collect_output(retained.len(), true) {
        collect_output(&mut retained, &mut scratch, true, |bytes| {
            source.read(bytes)
        })?;
    }
    assert_eq!(retained.len(), RETAIN_BYTES);
    assert_eq!(source.position, RETAIN_BYTES);
    let reads_at_capacity = source.reads;
    collect_output(&mut retained, &mut scratch, true, |bytes| {
        source.read(bytes)
    })?;
    assert_eq!(source.reads, reads_at_capacity);

    let mut delivered = Vec::new();
    // The consumer frees less than one complete message per iteration. This
    // checks the same capacity predicate used by the readiness subscription.
    while delivered.len() < source.bytes.len() {
        let count = retained.len().min(997);
        assert!(count != 0, "queued output stopped making progress");
        delivered.extend(retained.drain(..count));
        collect_output(&mut retained, &mut scratch, true, |bytes| {
            source.read(bytes)
        })?;
        assert!(retained.len() <= RETAIN_BYTES);
    }
    assert_eq!(delivered, source.bytes);
    assert!(retained.is_empty());
    Ok(())
}

#[test]
fn detaching_a_blocked_reader_restores_rolling_history_collection() -> Result<()> {
    let mut retained = VecDeque::from(vec![b'a'; RETAIN_BYTES]);
    let mut source = Output {
        bytes: b"latest".to_vec(),
        position: 0,
        reads: 0,
    };
    let mut scratch = [0; BATCH];
    collect_output(&mut retained, &mut scratch, true, |bytes| {
        source.read(bytes)
    })?;
    assert_eq!(source.reads, 0);
    collect_output(&mut retained, &mut scratch, false, |bytes| {
        source.read(bytes)
    })?;
    assert_eq!(source.position, 6);
    assert_eq!(retained.len(), RETAIN_BYTES);
    assert!(retained.make_contiguous().ends_with(b"latest"));
    Ok(())
}
