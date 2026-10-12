// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup and control-plane contracts for Native VM services.

use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, FileObject, Rights,
    VirtualMachineCreationLeaseObject,
};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const CREATION_LEASE_NAME: &str = "vm.creation-lease";
pub const CREATION_AUTHORITY_NAME: &str = "vm.creation-authority";
pub const IMAGE_NAME: &str = "vm.image";
pub const RUNTIME_IMAGE_NAME: &str = "vm.runtime-image";
pub const PROVISIONING_NAME: &str = "vm.provisioning";
pub const INSTANCE_CONTROL_NAME: &str = "vm.instance-control";
pub const CONSOLE_CONNECTION_NAME: &str = "vm.console-connection";
pub const MANAGER_CONNECTION_NAME: &str = "vm.manager-connection";
pub const CLIENT_CONTROL_NAME: &str = "vm.client-control";
pub const CLIENT_CAPABILITIES_NAME: &str = "vm.client-capabilities";

pub const CREATION_LEASE: StartupPurpose<VirtualMachineCreationLeaseObject> =
    StartupPurpose::new(0x8005_0001);
pub const IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0002);
pub const RUNTIME_IMAGE: StartupPurpose<FileObject> = StartupPurpose::new(0x8005_0003);
pub const PROVISIONING: StartupPurpose<CapabilityChannelObject> = StartupPurpose::new(0x8005_0004);
pub const INSTANCE_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0005);
pub const CONSOLE_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0006);
pub const MANAGER_CONNECTION: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0007);
pub const CLIENT_CONTROL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8005_0008);
pub const CLIENT_CAPABILITIES: StartupPurpose<CapabilityChannelObject> =
    StartupPurpose::new(0x8005_0009);

/// Rights retained while the manager duplicates a guest image into a runtime.
pub const MANAGED_IMAGE_RIGHTS: Rights = Rights::READ
    .union(Rights::DUPLICATE)
    .union(Rights::TRANSFER);
/// The fleet manager only reads its bootstrap configuration; it does not forward it.
pub const PROVISIONED_CONFIG_RIGHTS: Rights = Rights::READ;
/// The manager sends one configuration result, then closes this endpoint.
pub const PROVISIONED_RESULT_RIGHTS: Rights = Rights::WRITE;
/// Rights visible to one VM runtime for its instance-control endpoint.
pub const INSTANCE_CONTROL_RIGHTS: Rights = Rights::WAIT.union(Rights::READ).union(Rights::WRITE);
pub const RUNTIME_CONSOLE_CONNECTION_RIGHTS: Rights = Rights::WAIT.union(Rights::READ);
pub const CONSOLE_SESSION_RIGHTS: Rights = Rights::WAIT
    .union(Rights::READ)
    .union(Rights::WRITE)
    .union(Rights::TRANSFER);
pub const MANAGER_CONNECTION_RIGHTS: Rights = Rights::DUPLICATE
    .union(Rights::TRANSFER)
    .union(Rights::WAIT)
    .union(Rights::READ)
    .union(Rights::WRITE);

pub const MANAGER_RUNTIME_IMAGE_CONTRACT: StartupContract = StartupContract::exact(
    RUNTIME_IMAGE_NAME,
    RUNTIME_IMAGE,
    crate::process::EXECUTABLE_FILE_RIGHTS.as_rights(),
);
pub const MANAGER_PROVISIONING_CONTRACT: StartupContract = StartupContract::exact(
    PROVISIONING_NAME,
    PROVISIONING,
    Rights::WAIT.union(Rights::READ),
);
pub const MANAGER_CREATION_AUTHORITY_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_AUTHORITY_NAME,
    hyper_os::startup::VIRTUAL_MACHINE_CREATION_AUTHORITY,
    Rights::DERIVE.union(Rights::CREATE_VIRTUAL_MACHINE),
);
pub const MANAGER_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::READ),
);

pub const MANAGER_STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        crate::process::ROOT_DIRECTORY_NAME,
        hyper_os::startup::ROOT_DIRECTORY,
        Rights::READ
            .union(Rights::DUPLICATE)
            .union(Rights::TRANSFER),
    ),
    MANAGER_RUNTIME_IMAGE_CONTRACT,
    MANAGER_PROVISIONING_CONTRACT,
    MANAGER_CREATION_AUTHORITY_CONTRACT,
    MANAGER_CONNECTION_CONTRACT,
    crate::io::BROKER_CLIENT_CONTRACT,
];

pub const RUNTIME_IMAGE_CONTRACT: StartupContract =
    StartupContract::exact(IMAGE_NAME, IMAGE, Rights::READ);
pub const RUNTIME_CREATION_LEASE_CONTRACT: StartupContract = StartupContract::exact(
    CREATION_LEASE_NAME,
    CREATION_LEASE,
    Rights::CREATE_VIRTUAL_MACHINE.union(Rights::INSPECT),
);
pub const RUNTIME_INSTANCE_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    INSTANCE_CONTROL_NAME,
    INSTANCE_CONTROL,
    INSTANCE_CONTROL_RIGHTS,
);
pub const RUNTIME_CONSOLE_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_CONNECTION_NAME,
    CONSOLE_CONNECTION,
    RUNTIME_CONSOLE_CONNECTION_RIGHTS,
);

/// Complete startup vocabulary accepted by one per-VM runtime.
///
/// The connector receives authorized client byte channels from the manager.
pub const RUNTIME_STARTUP_CONTRACTS: &[StartupContract] = &[
    RUNTIME_IMAGE_CONTRACT,
    RUNTIME_CREATION_LEASE_CONTRACT,
    RUNTIME_INSTANCE_CONTROL_CONTRACT,
    RUNTIME_CONSOLE_CONNECTION_CONTRACT,
    crate::io::SESSION_CONTRACT,
];

pub const CLIENT_CONTROL_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CONTROL_NAME,
    CLIENT_CONTROL,
    Rights::WAIT.union(Rights::READ).union(Rights::WRITE),
);
pub const CLIENT_CAPABILITIES_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_CAPABILITIES_NAME,
    CLIENT_CAPABILITIES,
    Rights::WAIT.union(Rights::READ),
);
pub const CLIENT_STARTUP_CONTRACTS: &[StartupContract] = &[StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT.union(Rights::WRITE),
)];
/// A shell or console manager may pass this client connector to another trusted
/// shell. Individual vmm processes receive dedicated channels instead.
pub const DELEGATED_CLIENT_CONNECTION_CONTRACT: StartupContract = StartupContract::exact(
    MANAGER_CONNECTION_NAME,
    MANAGER_CONNECTION,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const VMM_STARTUP_CONTRACTS: &[StartupContract] =
    &[CLIENT_CONTROL_CONTRACT, CLIENT_CAPABILITIES_CONTRACT];

/// Connects a shell-launched client to the system VM manager.
/// The receiver supplies the control and capability channels atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagerConnectionRequest;

impl ManagerConnectionRequest {
    pub const CAPABILITY_COUNT: usize = 2;

    pub const fn encode(self) -> [u8; 8] {
        [b'H', b'V', b'M', 1, 5, 1, 0, 0]
    }
}
