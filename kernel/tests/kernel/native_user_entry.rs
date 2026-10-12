// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native machine probes using the production Process and VMO lifecycle.

#[cfg(CONFIG_ARCH_AARCH64)]
mod aarch64;
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
mod fp;
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(super) use fp::guest_program as fp_guest_program;
#[cfg(CONFIG_ARCH_AARCH64)]
mod atomic_wait;
#[cfg(CONFIG_ARCH_RISCV64)]
mod riscv64;
#[cfg(CONFIG_ARCH_AARCH64)]
pub(super) use aarch64::run;
#[cfg(CONFIG_ARCH_RISCV64)]
pub(super) use riscv64::run;

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::mm::user_space::{UserAddress, UserSlice, prepare_native_entry_self_test};
use crate::kernel::process::{
    MachineAbi, PreparedProcess, Process, ProcessImage, ProcessObject, ProcessPhase, TaskGroup,
};
use hyper::mm::PAGE_SIZE;

const IMAGE_BASE: u64 = 0x40_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AddressSpace,
    #[cfg(CONFIG_ARCH_AARCH64)]
    AtomicWait(&'static str),
    Construction,
    Group,
    Image,
    Lifecycle,
    Scheduler,
    Sleep(crate::kernel::task::SleepError),
    Terminal,
}

impl From<crate::kernel::task::SleepError> for Error {
    fn from(error: crate::kernel::task::SleepError) -> Self {
        Self::Sleep(error)
    }
}

fn prepare_process(
    domain: &ResourceDomain,
    group: &TaskGroup,
    program: &[u8],
    machine: MachineAbi,
) -> Result<Process, Error> {
    let code_range =
        UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE).map_err(|_| Error::Construction)?;
    let stack_range = UserSlice::new(UserAddress::new(IMAGE_BASE + PAGE_SIZE * 2), PAGE_SIZE)
        .map_err(|_| Error::Construction)?;
    let image_range = UserSlice::new(UserAddress::new(IMAGE_BASE), PAGE_SIZE * 3)
        .map_err(|_| Error::Construction)?;
    let address_space = prepare_native_entry_self_test(
        domain.clone(),
        image_range,
        code_range,
        stack_range,
        program,
    )
    .map_err(|_| Error::AddressSpace)?;
    let image = ProcessImage::try_native(
        machine,
        code_range.base(),
        stack_range.end(),
        UserAddress::new(0),
    )
    .map_err(|_| Error::Image)?;
    let prepared =
        match PreparedProcess::try_new(image, group.clone(), domain.clone(), address_space) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (_cause, address_space) = failure.into_parts();
                crate::kernel::mm::user_space::NativeAddressSpace::retire(address_space)
                    .map_err(|_| Error::AddressSpace)?;
                return Err(Error::Construction);
            }
        };
    let object = ProcessObject::try_service(prepared.process()).map_err(|_| Error::Construction)?;
    let process = prepared.publish(
        object,
        crate::kernel::process::ProcessNameSnapshot::from_validated("native-test"),
    );
    if !process_is_discoverable(&process) {
        return Err(Error::Construction);
    }
    process.start().map_err(|_| Error::Lifecycle)?;
    Ok(process)
}

fn process_is_discoverable(target: &Process) -> bool {
    let mut cursor = Some(crate::kernel::process::ProcessScanCursor::start());
    while let Some(position) = cursor {
        let page = crate::kernel::process::scan(position);
        if page
            .entries()
            .any(|entry| entry.snapshot().id == target.id())
        {
            return true;
        }
        cursor = page.next();
    }
    false
}

fn retire_process(process: &Process) -> Result<(), Error> {
    // Observation only: production kreaper must own and complete retirement.
    if crate::kernel::task::wait_for_test_progress(
        crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS,
        || Ok::<_, Error>(process.snapshot().phase == ProcessPhase::Retired),
    )? {
        Ok(())
    } else {
        Err(Error::Lifecycle)
    }
}

fn verify_thread_creation_affinity(
    domain: &ResourceDomain,
    group: &TaskGroup,
    program: &[u8],
    machine: MachineAbi,
) -> Result<(), Error> {
    let process = prepare_process(domain, group, program, machine)?;
    let caller = process
        .create_initial_user_thread(
            "selftest/affinity-parent",
            crate::kernel::task::scheduler::CpuMask::ALL,
        )
        .map_err(|_| Error::Scheduler)?;
    let result = crate::kernel::entry::verify_thread_affinity_creation_for_test(
        &process,
        &caller,
        UserAddress::new(IMAGE_BASE + PAGE_SIZE * 2),
    )
    .and_then(|()| crate::kernel::entry::verify_object_details_for_test(&process, &caller));
    process.request_stop(crate::kernel::process::TerminalReason::Requested);
    retire_process(&process)?;
    if let Err(reason) = result {
        crate::pr_err!("HypeR test: thread affinity creation failed: {reason}");
        return Err(Error::Scheduler);
    }
    crate::pr_info!("HypeR test: Native thread creation affinity passed");
    Ok(())
}

fn verify_admission_retry(
    domain: &ResourceDomain,
    group: &TaskGroup,
    program: &[u8],
    machine: MachineAbi,
    exit_status: i64,
) -> Result<(), Error> {
    use crate::kernel::process::TerminalReason;
    use crate::kernel::task::{TEST_PROGRESS_TIMEOUT_NS, wait_for_test_progress};

    // Cover both resumption after the updater reopens admission and process
    // cancellation while entry is retrying. This also runs on a single CPU:
    // retaining the entry pin would prevent this controller from reopening it.
    for cancel in [false, true] {
        let process = prepare_process(domain, group, program, machine)?;
        let result = (|| {
            let space = process
                .address_space_owner()
                .map_err(|_| Error::AddressSpace)?;
            let thread = process
                .create_initial_user_thread(
                    "selftest/admission-retry",
                    crate::kernel::task::scheduler::CpuMask::ALL,
                )
                .map_err(|_| Error::Construction)?;
            space.with_closed_admission_for_test(|| {
                thread.ready().map_err(|_| Error::Scheduler)?;
                if !wait_for_test_progress(TEST_PROGRESS_TIMEOUT_NS, || {
                    Ok::<_, Error>(space.admission_retries_for_test() >= 2)
                })? || process.try_join().is_some()
                {
                    return Err(Error::Lifecycle);
                }
                if cancel {
                    process.request_stop(TerminalReason::Requested);
                    if !wait_for_test_progress(TEST_PROGRESS_TIMEOUT_NS, || {
                        Ok::<_, Error>(thread.try_join().is_some())
                    })? {
                        return Err(Error::Lifecycle);
                    }
                }
                Ok(())
            })?;
            if !wait_for_test_progress(TEST_PROGRESS_TIMEOUT_NS, || {
                Ok::<_, Error>(process.try_join().is_some())
            })? {
                return Err(Error::Lifecycle);
            }
            let expected = if cancel {
                TerminalReason::Requested
            } else {
                TerminalReason::LastThreadExited {
                    status: exit_status,
                }
            };
            if process.try_join() != Some(expected) {
                return Err(Error::Terminal);
            }
            Ok(())
        })();
        process.request_stop(TerminalReason::Requested);
        retire_process(&process)?;
        result?;
    }
    crate::pr_info!("HypeR test: Native admission retry and cancellation passed");
    Ok(())
}
