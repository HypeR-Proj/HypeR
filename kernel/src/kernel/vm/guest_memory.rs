// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded Native device-backend access to retained guest RAM capabilities.

use super::objects::GuestMemoryObject;
use super::service::Error;
use crate::kernel::authority::Rights;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::{GuestMemoryBacking, UserSlice};
use crate::kernel::process::Process;
use alloc::vec::Vec;

pub(crate) fn read(
    process: &Process,
    handle: HandleValue,
    offset: u64,
    output: Option<UserSlice>,
) -> Result<(), Error> {
    let memory = process.resolve_handle::<GuestMemoryObject>(handle, Rights::READ)?;
    let access = memory.object().begin_native_access().ok_or(Error::Busy)?;
    let backing = access.backing();
    let length = validate_range(backing, offset, output)?;
    let Some(output) = output else { return Ok(()) };
    let mut small = [0; 8];
    let mut allocation = Vec::new();
    let bytes = transfer_buffer(length, &mut small, &mut allocation)?;
    populate(backing, offset, length)?;
    backing
        .read_native(offset, bytes)
        .map_err(super::service::classify_memory_object_error)?;
    process.copy_to_user(output, bytes)?;
    Ok(())
}

pub(crate) fn write(
    process: &Process,
    handle: HandleValue,
    offset: u64,
    input: Option<UserSlice>,
) -> Result<(), Error> {
    let memory = process.resolve_handle::<GuestMemoryObject>(handle, Rights::WRITE)?;
    let access = memory.object().begin_native_access().ok_or(Error::Busy)?;
    let backing = access.backing();
    let length = validate_range(backing, offset, input)?;
    let Some(input) = input else { return Ok(()) };
    let mut small = [0; 8];
    let mut allocation = Vec::new();
    let bytes = transfer_buffer(length, &mut small, &mut allocation)?;
    // User faults precede all externally visible writes. No VM/page lock is
    // held across this copy; the resolved handle and admission retain backing.
    process.copy_from_user(input, bytes)?;
    populate(backing, offset, length)?;
    backing
        .write_native(offset, bytes)
        .map_err(super::service::classify_memory_object_error)?;
    Ok(())
}

fn validate_range(
    backing: &GuestMemoryBacking,
    offset: u64,
    bytes: Option<UserSlice>,
) -> Result<usize, Error> {
    let length = bytes.map_or(0, UserSlice::length);
    if length > hyper::abi::native::HYPER_NATIVE_GUEST_MEMORY_MAX_TRANSFER_BYTES
        || offset
            .checked_add(length)
            .is_none_or(|end| end > backing.size())
    {
        return Err(Error::InvalidArgument);
    }
    usize::try_from(length).map_err(|_| Error::InvalidArgument)
}

fn transfer_buffer<'a>(
    length: usize,
    small: &'a mut [u8; 8],
    allocation: &'a mut Vec<u8>,
) -> Result<&'a mut [u8], Error> {
    if length <= small.len() {
        return Ok(&mut small[..length]);
    }
    crate::kernel::mm::reclaim::reserve_exact(allocation, length).map_err(|_| Error::NoMemory)?;
    allocation.resize(length, 0);
    Ok(allocation)
}

fn populate(backing: &GuestMemoryBacking, offset: u64, length: usize) -> Result<(), Error> {
    let page_size = hyper::mm::PAGE_SIZE;
    // validate_range bounded the end before this helper. Partial allocation
    // failure leaves committed pages owned and charged by the original VMO.
    let end = offset + length as u64;
    for page in ((offset & !(page_size - 1))..end).step_by(page_size as usize) {
        if !backing
            .page_is_resident(page)
            .map_err(super::service::classify_memory_object_error)?
        {
            backing
                .populate_page(page)
                .map_err(super::service::classify_memory_object_error)?;
        }
    }
    Ok(())
}
