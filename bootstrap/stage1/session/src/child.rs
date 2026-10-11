// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Construct a fresh shell client without transferring the console itself.

use hyper_os::fs::Directory;
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, Rights, RightsOffer,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::ProcessBuilder;
use hyper_service::{process, stdio};

pub struct ControlChannels {
    pub vm: Option<OwnedHandle<CapabilityChannelObject>>,
    pub filesystem: Option<OwnedHandle<CapabilityChannelObject>>,
}

pub fn launch(
    startup: &Startup<'_>,
    root: &Directory,
    environment: &Directory,
    mode: hyper_service::session::ShellMode,
    controls: &ControlChannels,
    channels: [OwnedHandle<ByteChannelObject>; 3],
) -> hyper_os::Result<OwnedHandle<ProcessObject>> {
    let [input, output, error] = channels;
    let system = if mode == hyper_service::session::ShellMode::System {
        Some(process::open_environment(root, "/system")?)
    } else {
        None
    };
    let image = if system.is_some() {
        "bin/sh"
    } else {
        "svc/fallback-sh"
    };
    let image_environment = system.as_ref().unwrap_or(environment);
    let executable =
        image_environment.open(image, hyper_service::process::EXECUTABLE_FILE_RIGHTS)?;
    let loader = process::open_loader(
        image_environment,
        if system.is_some() {
            process::LoaderKind::Dynamic
        } else {
            process::LoaderKind::Bootstrap
        },
    )?;
    let libraries = process::open_library_directory(image_environment)?;
    let builder = ProcessBuilder::create(
        startup.borrow(startup::TASK_FACTORY)?,
        startup.borrow(startup::TASK_GROUP)?,
        startup.borrow(startup::RESOURCE_DOMAIN)?,
        executable.as_handle_ref(),
        loader.as_handle_ref(),
    )?;
    builder.set_name(if system.is_some() {
        "shell"
    } else {
        "fallback-sh"
    })?;
    builder.add_argument(image)?;
    macro_rules! delegate {
        ($purpose:expr, $contract:expr) => {
            builder.add_handle_duplicate(
                startup.borrow($purpose)?,
                $purpose.as_raw(),
                RightsOffer::Exact($contract.allowed_rights()),
            )?;
        };
    }
    builder.add_handle_duplicate(
        root.as_handle_ref(),
        startup::ROOT_DIRECTORY.as_raw(),
        RightsOffer::Exact(process::SHELL_ROOT_DIRECTORY_CONTRACT.allowed_rights()),
    )?;
    delegate!(startup::TASK_FACTORY, process::SHELL_TASK_FACTORY_CONTRACT);
    delegate!(startup::TASK_GROUP, process::SHELL_TASK_GROUP_CONTRACT);
    delegate!(
        startup::RESOURCE_DOMAIN,
        process::SHELL_RESOURCE_DOMAIN_CONTRACT
    );
    process::ShellDelegation {
        environment: environment.as_handle_ref(),
        loader: startup.borrow(process::CHILD_LOADER)?,
        libraries: startup.borrow(process::CHILD_LIBRARY_DIRECTORY)?,
        tasks: startup.borrow(startup::TASK_INSPECTOR)?,
        objects: startup.borrow(startup::OBJECT_INSPECTOR)?,
        memory: startup.borrow(startup::MEMORY_INSPECTOR)?,
        cpus: startup.borrow(startup::CPU_INSPECTOR)?,
        filesystem: controls.filesystem.as_ref().map(OwnedHandle::as_handle_ref),
        virtual_machines: controls.vm.as_ref().map(OwnedHandle::as_handle_ref),
    }
    .add_to(&builder)?;
    builder.add_handle_duplicate(
        libraries.as_handle_ref(),
        startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
        RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE).union(Rights::PROTECT)),
    )?;
    builder.add_handle_duplicate(
        input.as_handle_ref(),
        stdio::TERMINAL_INPUT.as_raw(),
        RightsOffer::Exact(stdio::TERMINAL_INPUT_CONTRACT.allowed_rights()),
    )?;
    for (channel, contract) in [
        (input, stdio::STANDARD_INPUT_CONTRACT),
        (output, stdio::STANDARD_OUTPUT_CONTRACT),
        (error, stdio::STANDARD_ERROR_CONTRACT),
    ] {
        builder
            .add_handle_move(
                channel,
                contract.purpose(),
                RightsOffer::Exact(contract.allowed_rights()),
            )
            .map_err(|failure| failure.error())?;
    }
    builder.seal()?;
    builder.start().map_err(|failure| failure.error())
}
