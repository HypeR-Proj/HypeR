// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native vm service calls under borrowed Process authority.

use super::ProcessServices;
use super::affinity::AffinityInputError;
use crate::kernel::abi::native::VmServices;
use crate::kernel::capability::{HandleValue, InvocationReceiver, Rights};
use crate::kernel::mm::user_space::UserSlice;

impl ProcessServices<'_> {
    // Keep the full batch buffer off the interactive input call path. Both
    // variants copy once before publishing any input and kick the guest once.
    // A full 4 KiB write still needs its complete scratch buffer: shortening
    // that batch would change partial-write behavior and increase syscall cost.
    #[inline(never)]
    fn copy_virtual_serial_input<const CAPACITY: usize>(
        &self,
        serial: &crate::kernel::vm::virtual_serial::VirtualSerial,
        source: UserSlice,
        length: usize,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        let mut bytes = [0; CAPACITY];
        let bytes = bytes
            .get_mut(..length)
            .ok_or(crate::kernel::vm::service::Error::InvalidArgument)?;
        self.process.copy_from_user(source, bytes)?;
        serial
            .write_input(bytes)
            .map_err(crate::kernel::vm::objects::Error::from)
            .map_err(Into::into)
    }
}

impl VmServices for ProcessServices<'_> {
    fn virtual_machine_platform_info(
        &self,
        lease: &InvocationReceiver,
        profile: u32,
    ) -> Result<
        crate::kernel::vm::service::VirtualMachinePlatformInfo,
        crate::kernel::vm::service::Error,
    > {
        crate::kernel::vm::service::platform_info(lease, profile)
    }

    fn derive_virtual_machine_creation_lease(
        &self,
        authority: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::derive_creation_lease(self.process, authority, domain)
    }

    fn create_pending_virtual_machine(
        &self,
        lease: &InvocationReceiver,
        configuration: crate::kernel::vm::objects::VirtualMachineConfiguration,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_pending(self.process, lease, configuration)
    }

    fn set_pending_virtual_machine_memory(
        &self,
        pending: &InvocationReceiver,
        vmo: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_memory(self.process, pending, vmo)
    }

    fn set_pending_virtual_machine_bootstrap(
        &self,
        pending: &InvocationReceiver,
        bootstrap: crate::kernel::vm::objects::VirtualCpuBootstrap,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_bootstrap(pending, bootstrap)
    }

    fn set_pending_virtual_machine_virtual_serial(
        &self,
        pending: &InvocationReceiver,
        serial: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_virtual_serial(self.process, pending, serial)
    }

    fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_virtual_serial(self.process)
    }

    fn register_virtual_serial_output(
        &self,
        serial: &InvocationReceiver,
        buffer: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_virtual_serial_output(self.process, serial, buffer)
    }
    fn acknowledge_virtual_serial_output(
        &self,
        serial: &InvocationReceiver,
        consumed: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::acknowledge_virtual_serial_output(serial, consumed)
    }
    fn write_virtual_serial(
        &self,
        value: &InvocationReceiver,
        source: Option<UserSlice>,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        let serial = value
            .resolve::<crate::kernel::vm::virtual_serial::VirtualSerial>(Rights::WRITE)
            .map_err(crate::kernel::process::ProcessError::from)?;
        let Some(source) = source else {
            return Ok(0);
        };
        let length = usize::try_from(source.length())
            .map_err(|_| crate::kernel::vm::service::Error::InvalidArgument)?
            .min(crate::kernel::vm::virtual_serial::TRANSFER_BATCH_BYTES);
        let length_bytes =
            u64::try_from(length).map_err(|_| crate::kernel::vm::service::Error::Internal)?;
        let source = UserSlice::new(source.base(), length_bytes)
            .map_err(|_| crate::kernel::vm::service::Error::Fault)?;
        if length <= 128 {
            self.copy_virtual_serial_input::<128>(serial.object(), source, length)
        } else {
            self.copy_virtual_serial_input::<
                { crate::kernel::vm::virtual_serial::TRANSFER_BATCH_BYTES },
            >(serial.object(), source, length)
        }
    }

    fn seal_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::seal(pending)
    }

    fn install_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::install(self.process, pending)
    }

    fn abort_pending_virtual_machine(
        &self,
        pending: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::abort(self.process, pending)
    }

    fn request_virtual_machine_stop(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::request_stop(machine)
    }

    fn register_mmio(
        &self,
        machine: &InvocationReceiver,
        base: u64,
        length: u64,
        device: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_mmio(machine, base, length, device)
    }
    fn pending_mmio(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<Option<hyper::vm::device::mmio::Request>, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::pending_mmio(vcpu)
    }
    fn complete_mmio(
        &self,
        vcpu: &InvocationReceiver,
        id: u64,
        action: hyper::vm::exit::MmioAction,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::complete_mmio(vcpu, id, action)
    }
    fn create_guest_memory(
        &self,
        vmo: &InvocationReceiver,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_guest_memory(self.process, vmo)
    }
    fn map_guest_memory(
        &self,
        pending: &InvocationReceiver,
        memory: HandleValue,
        guest_offset: u64,
        source_offset: u64,
        length: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::map_guest_memory(
            self.process,
            pending,
            memory,
            guest_offset,
            source_offset,
            length,
        )
    }
    fn pending_power_request(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<Option<hyper::vm::arm::psci::Request>, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::pending_power_request(machine)
    }
    fn complete_power_request(
        &self,
        machine: &InvocationReceiver,
        request_id: u64,
        accept: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::complete_power_request(machine, request_id, accept)
    }
    fn open_vcpu(
        &self,
        machine: &InvocationReceiver,
        vcpu: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::open_vcpu(self.process, machine, vcpu)
    }
    fn virtual_machine_info(
        &self,
        machine: &InvocationReceiver,
    ) -> Result<
        (
            crate::kernel::vm::objects::VirtualMachineConfiguration,
            crate::kernel::vm::objects::VirtualMachineSnapshot,
        ),
        crate::kernel::vm::service::Error,
    > {
        crate::kernel::vm::service::machine_info(machine)
    }

    fn virtual_cpu_info(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>
    {
        crate::kernel::vm::service::vcpu_info(vcpu)
    }

    fn set_virtual_cpu_affinity(
        &self,
        vcpu: &InvocationReceiver,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        let affinity = self
            .copy_affinity(words, word_count)
            .map_err(|error| match error {
                AffinityInputError::Invalid => crate::kernel::vm::service::Error::InvalidArgument,
                AffinityInputError::Memory(error) => crate::kernel::vm::service::Error::from(error),
            })?;
        crate::kernel::vm::service::set_vcpu_affinity(vcpu, affinity)
    }

    fn start_virtual_cpu(
        &self,
        vcpu: &InvocationReceiver,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::start_vcpu(vcpu)
    }
}
