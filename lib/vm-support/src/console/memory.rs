// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest RAM authority retained independently of guest execution and teardown.

use crate::virtio_console::{Error, GuestMemory};
use hyper_os::handle::{GuestMemoryObject, OwnedHandle};

pub(super) struct Memory {
    pub(super) grant: OwnedHandle<GuestMemoryObject>,
    pub(super) base: u64,
}

impl GuestMemory for Memory {
    fn read(&self, address: u64, bytes: &mut [u8]) -> Result<(), Error> {
        let offset = address.checked_sub(self.base).ok_or(Error::Memory)?;
        hyper_os::vm::read_guest_memory(self.grant.as_handle_ref(), offset, bytes)
            .map_err(|_| Error::Memory)
    }

    fn write(&self, address: u64, bytes: &[u8]) -> Result<(), Error> {
        let offset = address.checked_sub(self.base).ok_or(Error::Memory)?;
        hyper_os::vm::write_guest_memory(self.grant.as_handle_ref(), offset, bytes)
            .map_err(|_| Error::Memory)
    }

    fn load_u16(&self, address: u64) -> Result<u16, Error> {
        if address & 1 != 0 {
            return Err(Error::Memory);
        }
        let mut bytes = [0; 2];
        self.read(address, &mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn store_u16(&self, address: u64, value: u16) -> Result<(), Error> {
        if address & 1 != 0 {
            return Err(Error::Memory);
        }
        self.write(address, &value.to_le_bytes())
    }
}
