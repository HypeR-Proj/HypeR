// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Backend-copy authority, sparse backing, and aligned shared-word acceptance.

use hyper_os::handle::Rights;
use hyper_os::memory::WritableVmo;
use hyper_os::{Error, Status, vm};
use std::sync::Arc;

type Result<T> = std::result::Result<T, String>;

fn show(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub(super) fn suite() -> Result<()> {
    const BYTES: u64 = 128 * 1024;
    let memory = WritableVmo::create(BYTES).map_err(show)?;
    let grant = vm::create_guest_memory(memory.as_handle_ref()).map_err(show)?;
    let read_only = grant.duplicate(Rights::READ).map_err(show)?;
    let write_only = grant.duplicate(Rights::WRITE).map_err(show)?;
    let map_only = grant.duplicate(Rights::MAP).map_err(show)?;
    for result in [
        vm::write_guest_memory(read_only.as_handle_ref(), 0, &[1]),
        vm::read_guest_memory(write_only.as_handle_ref(), 0, &mut [0]),
        vm::write_guest_memory(map_only.as_handle_ref(), 0, &[]),
        vm::read_guest_memory(map_only.as_handle_ref(), 0, &mut []),
    ] {
        require_status(result, Status::ACCESS_DENIED)?;
    }
    for result in [
        vm::read_guest_memory(grant.as_handle_ref(), BYTES, &mut [0]),
        vm::write_guest_memory(grant.as_handle_ref(), BYTES + 1, &[]),
        vm::write_guest_memory(grant.as_handle_ref(), u64::MAX, &[1]),
    ] {
        require_status(result, Status::INVALID_ARGUMENT)?;
    }
    let too_large = vec![0; vm::GUEST_MEMORY_MAX_TRANSFER_BYTES + 1];
    require_status(
        vm::write_guest_memory(grant.as_handle_ref(), 0, &too_large),
        Status::INVALID_ARGUMENT,
    )?;
    vm::read_guest_memory(grant.as_handle_ref(), BYTES, &mut []).map_err(show)?;
    // An untouched sparse page must read zero and become stable backing.
    let mut zero = [1; 8];
    vm::read_guest_memory(grant.as_handle_ref(), 0, &mut zero).map_err(show)?;
    if zero != [0; 8] {
        return Err("guest sparse read was not zero-filled".into());
    }
    // Cross a page boundary with a non-scalar copy, then close the source VMO.
    let pattern: Vec<u8> = (0..8193).map(|index| (index % 251) as u8).collect();
    vm::write_guest_memory(grant.as_handle_ref(), 4093, &pattern).map_err(show)?;
    drop(memory);
    let mut output = vec![0; pattern.len()];
    vm::read_guest_memory(grant.as_handle_ref(), 4093, &mut output).map_err(show)?;
    if output != pattern {
        return Err("guest cross-page copy or grant ownership failed".into());
    }
    // Guest scalar atomicity is independent of an unaligned Native buffer.
    for width in [1, 2, 4, 8] {
        let mut native = [0xa5; 16];
        vm::write_guest_memory(grant.as_handle_ref(), 16384, &native[1..1 + width])
            .map_err(show)?;
        native.fill(0);
        vm::read_guest_memory(grant.as_handle_ref(), 16384, &mut native[1..1 + width])
            .map_err(show)?;
        if native[1..1 + width] != [0xa5; 8][..width] {
            return Err("guest scalar width conversion failed".into());
        }
    }
    concurrent_indices(Arc::new(grant))?;
    println!("HypeR vm-smoke: guest memory copy authority and scalar concurrency passed");
    Ok(())
}

fn concurrent_indices(
    grant: Arc<hyper_os::OwnedHandle<hyper_os::handle::GuestMemoryObject>>,
) -> Result<()> {
    const OFFSET: u64 = 32768;
    const FIRST: u16 = 0x00ff;
    const SECOND: u16 = 0xff00;
    vm::write_guest_memory(grant.as_handle_ref(), OFFSET, &FIRST.to_le_bytes()).map_err(show)?;
    let writer_grant = grant.clone();
    let writer = std::thread::Builder::new()
        .name("guest-copy-writer".into())
        .spawn(move || -> Result<()> {
            for index in 0..1024 {
                let value = if index & 1 == 0 { FIRST } else { SECOND };
                vm::write_guest_memory(writer_grant.as_handle_ref(), OFFSET, &value.to_le_bytes())
                    .map_err(show)?;
            }
            Ok(())
        })
        .map_err(show)?;
    let reader: Result<()> = (|| {
        for _ in 0..1024 {
            let mut bytes = [0; 2];
            vm::read_guest_memory(grant.as_handle_ref(), OFFSET, &mut bytes).map_err(show)?;
            if !matches!(u16::from_le_bytes(bytes), FIRST | SECOND) {
                return Err("guest shared u16 access tore".into());
            }
        }
        Ok(())
    })();
    // Always join, including read failures, before releasing test-owned state.
    let writer = writer
        .join()
        .map_err(|_| "guest copy writer failed".to_string())?;
    reader?;
    writer
}

fn require_status(result: hyper_os::Result<()>, expected: Status) -> Result<()> {
    match result {
        Err(Error::Status(status)) if status == expected => Ok(()),
        other => Err(format!("expected {expected:?}, received {other:?}")),
    }
}
