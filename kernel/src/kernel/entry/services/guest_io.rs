// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native guest I/O service calls under borrowed Process authority.

use super::ProcessServices;
use crate::kernel::capability::{HandleValue, InvocationReceiver};
use crate::kernel::mm::user_space::UserSlice;

impl crate::kernel::abi::native::GuestIoServices for ProcessServices<'_> {
    fn transfer_guest_memory(
        &self,
        memory: &InvocationReceiver,
        write: bool,
        offset: u64,
        buffer: UserSlice,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::objects::guest_memory_access::transfer(
            self.process,
            memory,
            write,
            offset,
            buffer,
        )
    }
    fn scalar_guest_memory(
        &self,
        memory: &InvocationReceiver,
        write: bool,
        offset: u64,
        width: u32,
        value: u64,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        crate::kernel::vm::objects::guest_memory_access::scalar(memory, write, offset, width, value)
    }
    fn create_guest_interrupt(
        &self,
        machine: &InvocationReceiver,
        irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_interrupt(self.process, machine, irq)
    }
    fn set_guest_interrupt_level(
        &self,
        interrupt: &InvocationReceiver,
        asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::set_interrupt_level(interrupt, asserted)
    }

    fn create_guest_mapping(
        &self,
        backend: &InvocationReceiver,
        memory: HandleValue,
        frontend: u64,
    ) -> Result<(HandleValue, u64), crate::kernel::vm::service::Error> {
        crate::kernel::vm::create_guest_mapping(self.process, backend, memory, frontend)
    }
    fn release_guest_mapping(
        &self,
        mapping: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::release_guest_mapping(mapping)
    }
    fn create_native_block(
        &self,
        memory: &InvocationReceiver,
        backend: HandleValue,
        guest_base: u64,
        notification_base: u64,
        notification_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::block::service::create(
            self.process,
            memory,
            backend,
            guest_base,
            notification_base,
            notification_irq,
        )
    }
    fn activate_native_block(
        &self,
        block: &InvocationReceiver,
        readonly: bool,
    ) -> Result<(u64, bool), crate::kernel::block::service::ActivationError> {
        crate::kernel::block::service::activate(block, readonly)
    }
    fn transfer_native_block(
        &self,
        block: &InvocationReceiver,
        operation: u32,
        first: u64,
        buffer: Option<UserSlice>,
    ) -> Result<(), crate::kernel::block::service::TransferError> {
        crate::kernel::block::service::transfer(self.process, block, operation, first, buffer)
    }
    fn mount_filesystem(
        &self,
        channel: HandleValue,
        buffer: HandleValue,
        directory: &InvocationReceiver,
        path: UserSlice,
        read_only: bool,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        crate::kernel::vfs::service::mount_remote(
            self.process,
            channel,
            buffer,
            directory,
            path,
            read_only,
        )
    }
    fn unmount_filesystem(
        &self,
        directory: &InvocationReceiver,
        path: UserSlice,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        crate::kernel::vfs::service::unmount(self.process, directory, path)
    }

    fn create_guest_mailbox(
        &self,
        machine: &InvocationReceiver,
        base: u64,
        irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_mailbox(self.process, machine, base, irq)
    }
    fn send_guest_mailbox(
        &self,
        mailbox: &InvocationReceiver,
        bytes: &[u8],
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::send_mailbox(mailbox, bytes)
    }
    fn receive_guest_mailbox(
        &self,
        mailbox: &InvocationReceiver,
        copy: &mut dyn FnMut(&[u8]) -> Result<(), crate::kernel::vm::service::Error>,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::receive_mailbox(mailbox, copy)
    }
    fn create_guest_notification(
        &self,
        frontend: &InvocationReceiver,
        backend: HandleValue,
        frontend_base: u64,
        backend_base: u64,
        frontend_irq: u32,
        backend_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_notification(
            self.process,
            frontend,
            backend,
            frontend_base,
            backend_base,
            frontend_irq,
            backend_irq,
        )
    }
    fn access_guest_notification(
        &self,
        machine: &InvocationReceiver,
        original: u64,
        operation: u32,
        value: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::access_notification(machine, original, operation, value)
    }
    fn configure_guest_notification(
        &self,
        machine: &InvocationReceiver,
        original: u64,
        base: u64,
        flags: u32,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::configure_notification(machine, original, base, flags)
    }
    fn control_guest_notification(
        &self,
        notification: &InvocationReceiver,
        operation: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::control_notification(notification, operation)
    }
}
