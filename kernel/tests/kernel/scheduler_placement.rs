// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial CPU distribution and wakeup locality with real scheduler contexts.

use hyper::cpu::{CpuIndex, MAX_CPUS};
use hyper::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::task::scheduler::{self, CpuMask, ThreadPriority};

static OBSERVED: [AtomicUsize; MAX_CPUS] = [const { AtomicUsize::new(0) }; MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Scheduler(scheduler::Error),
    Sleep(crate::kernel::task::SleepError),
    Quiescence(super::support::QuiescenceError),
    Placement,
}

impl From<scheduler::Error> for Error {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<crate::kernel::task::SleepError> for Error {
    fn from(error: crate::kernel::task::SleepError) -> Self {
        Self::Sleep(error)
    }
}

impl From<super::support::QuiescenceError> for Error {
    fn from(error: super::support::QuiescenceError) -> Self {
        Self::Quiescence(error)
    }
}

pub(super) fn run() -> Result<(), Error> {
    let count = crate::kernel::cpu::online_cpu_count();
    for _ in 0..2 {
        let mut assigned = [0; MAX_CPUS];
        let mut seen = CpuMask::EMPTY;
        for (index, expected) in assigned.iter_mut().enumerate().take(count) {
            OBSERVED[index].store(0, Ordering::Relaxed);
            let worker = if index % 2 == 0 {
                scheduler::kthread_create("placement/fair", worker, index)?
            } else {
                scheduler::kthread_create_fifo(
                    "placement/fifo",
                    worker,
                    index,
                    ThreadPriority::NORMAL,
                )?
            };
            let (cpu, affinity) = scheduler::thread_placement(worker)?;
            if affinity != CpuMask::ALL || seen.contains(cpu) {
                return Err(Error::Placement);
            }
            seen = seen.with_cpu(cpu);
            *expected = cpu.get() + 1;
            scheduler::thread_ready(worker)?;

            // A service bound to CPU 0 between every ordinary creation must
            // not reset the rotation for the remaining work.
            let pinned = scheduler::kthread_create_with_affinity(
                "placement/pinned",
                worker_entry_unused,
                0,
                CpuMask::single(CpuIndex::BOOT),
            )?;
            if scheduler::thread_placement(pinned)?.0 != CpuIndex::BOOT {
                return Err(Error::Placement);
            }
            scheduler::discard_dormant_kernel_thread(pinned)?;
        }
        if !crate::kernel::task::wait_for_test_progress(
            crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS,
            || {
                Ok::<_, Error>(
                    OBSERVED
                        .iter()
                        .take(count)
                        .all(|cpu| cpu.load(Ordering::Acquire) != 0),
                )
            },
        )? {
            return Err(Error::Placement);
        }
        super::support::quiesce_workers()?;
        for (observed, expected) in OBSERVED.iter().zip(assigned).take(count) {
            if observed.load(Ordering::Acquire) != expected {
                return Err(Error::Placement);
            }
        }
    }
    crate::pr_info!("HypeR test: initial thread CPU distribution and wakeup locality passed");
    Ok(())
}

extern "C" fn worker(index: usize) {
    let before = crate::kernel::cpu::current_index();
    let success = scheduler::yield_now().is_ok()
        && crate::kernel::task::sleep_ms(1).is_ok()
        && before == crate::kernel::cpu::current_index();
    let observed = before
        .filter(|_| success)
        .map_or(usize::MAX, |cpu| cpu.get() + 1);
    OBSERVED[index].store(observed, Ordering::Release);
}

extern "C" fn worker_entry_unused(_: usize) {}
