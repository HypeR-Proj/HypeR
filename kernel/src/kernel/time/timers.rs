// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-CPU software timers multiplexed onto one architectural comparator.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicBool, Ordering};
use hyper::time::{OwnedDeadlineQueue, PendingTimer, TimerQueueError};

use crate::kernel::task::scheduler;

mod irq_callout;
pub(crate) use irq_callout::{ArmedIrqCallout, IrqCallout, OwnedArmedIrqCallout};

pub use hyper::time::{QueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode};

/// Bound one interrupt's callback work without imposing a timer storage cap.
const MAX_CALLBACKS_PER_INTERRUPT: usize = 64;

type TimerLock = InterruptSpinLock<ProcessorTimers, crate::hal::irq::LocalMask>;

static TIMERS: PerCpu<TimerLock> =
    PerCpu::new([const { TimerLock::new(ProcessorTimers::new()) }; hyper::cpu::MAX_CPUS]);

static REPROGRAM: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);

struct ProcessorTimers {
    initialized: bool,
    queue: OwnedDeadlineQueue,
    ready: super::callout::ReadyQueue,
}

impl ProcessorTimers {
    const fn new() -> Self {
        Self {
            initialized: false,
            queue: OwnedDeadlineQueue::new(),
            ready: super::callout::ReadyQueue::new(),
        }
    }
}

pub(super) fn initialize_local() -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        if timers.initialized {
            return Err(super::Error::TimerQueueAlreadyInitialized);
        }
        crate::hal::time::disable_local_timer();
        timers.queue.initialize_id(cpu.get())?;
        timers.initialized = true;
        Ok(())
    })
}

pub fn schedule_irq_at(
    deadline: u64,
    mode: TimerMode,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let pending = PendingTimer::try_new(deadline, mode, callback, context)?;
    // Allocation must precede pinning: allocator slow paths may reach a
    // scheduling point, while the selected queue and comparator are CPU-local.
    let (cpu_pin, cpu) = pin_current_timer_cpu()?;
    if let Err(error) = TIMERS[cpu].with(|timers| ensure_initialized(timers)) {
        release_timer_cpu_pin(cpu_pin);
        return Err(error);
    }
    let (result, retired, rollback_failed) = TIMERS[cpu].with(|timers| {
        let previous = timers.queue.next_deadline();
        let handle = timers.queue.insert(pending, super::monotonic_ticks());
        if timers.queue.next_deadline() != previous
            && let Err(error) = program_next(&timers.queue)
        {
            let retired = match timers.queue.cancel(handle) {
                Ok(retired) => Some(retired),
                // The exact handle was inserted while this lock was held. If
                // rollback cannot find it, returning would abandon a possibly
                // live callback context. Report the invariant only after the
                // timer lock and local interrupt mask have been released.
                Err(_) => return (Err(error), None, true),
            };
            return (Err(error), retired, false);
        }
        (Ok(handle), None, false)
    });
    release_timer_cpu_pin(cpu_pin);
    // Queue retirement may enter the allocator and therefore happens only
    // after the CPU-local comparator transaction is complete and unpinned.
    drop(retired);
    if rollback_failed {
        // The caller that supplied `context` cannot observe a return, so its
        // callback owner remains live while coordinated crash handling stops
        // every CPU that could consume the corrupt timer queue.
        crate::kernel::crash::fatal(format_args!("HypeR: exact timer insertion rollback failed"));
    }
    result
}

pub fn schedule_irq_after(
    nanoseconds: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let deadline = super::deadline_after(nanoseconds)?;
    schedule_irq_at(deadline, TimerMode::OneShot, callback, context)
}

pub fn schedule_irq_periodic(
    first_deadline: u64,
    interval_ticks: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    schedule_irq_at(
        first_deadline,
        TimerMode::Periodic {
            interval: interval_ticks,
        },
        callback,
        context,
    )
}

/// Cancels a timer on the queue identified by its handle.
///
/// A Thread may resume on a different CPU from the one on which it armed a
/// timer. Cancellation therefore follows the handle back to its source queue.
/// Only the source CPU can update its architectural comparator: remote
/// cancellation may leave an obsolete earlier deadline programmed, causing at
/// most one harmless timer interrupt which then programs the next deadline.
pub fn cancel_irq_timer(handle: TimerHandle) -> Result<(), super::Error> {
    let owner = CpuIndex::new(handle.queue_id()).ok_or(super::Error::InvalidCpuIndex)?;
    let (cpu_pin, current) = pin_current_timer_cpu()?;
    let cancelled = TIMERS[owner].with(|timers| {
        ensure_initialized(timers)?;
        let previous = timers.queue.next_deadline();
        let retired = timers.queue.cancel(handle)?;
        let result = if owner == current && timers.queue.next_deadline() != previous {
            program_next(&timers.queue)
        } else {
            Ok(())
        };
        Ok::<_, super::Error>((result, retired))
    });
    release_timer_cpu_pin(cpu_pin);
    let (result, retired) = cancelled?;
    drop(retired);
    result
}

pub fn local_statistics() -> Option<QueueStats> {
    let (cpu_pin, cpu) = pin_current_timer_cpu().ok()?;
    let statistics = TIMERS[cpu].with(|timers| timers.initialized.then(|| timers.queue.stats()));
    release_timer_cpu_pin(cpu_pin);
    statistics
}

pub(super) fn handle_interrupt() -> Result<usize, super::Error> {
    let cpu = current_cpu()?;
    crate::hal::time::mask_local_timer();
    let mut callbacks = 0;
    while callbacks < MAX_CALLBACKS_PER_INTERRUPT {
        let expired = TIMERS[cpu].with(|timers| {
            ensure_initialized(timers)?;
            Ok::<_, super::Error>(timers.queue.pop_expired(super::monotonic_ticks()))
        })?;
        let Some(expired) = expired else {
            break;
        };
        expired.invoke();
        callbacks += 1;
    }
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        program_next(&timers.queue)
    })?;
    Ok(callbacks)
}

fn current_cpu() -> Result<CpuIndex, super::Error> {
    crate::kernel::cpu::current_index().ok_or(super::Error::InvalidCpuIndex)
}

/// Pins the calling continuation before selecting one CPU-local timer queue.
fn pin_current_timer_cpu() -> Result<(scheduler::PreemptionGuard, CpuIndex), super::Error> {
    let cpu_pin = scheduler::preempt_disable().map_err(super::Error::Preemption)?;
    match current_cpu() {
        Ok(cpu) => Ok((cpu_pin, cpu)),
        Err(error) => {
            release_timer_cpu_pin(cpu_pin);
            Err(error)
        }
    }
}

/// Ends a timer CPU-local transaction without adding a scheduling point.
fn release_timer_cpu_pin(cpu_pin: scheduler::PreemptionGuard) {
    if scheduler::preempt_enable_without_reschedule(cpu_pin).is_err() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: timer CPU-local preemption pin release failed"
        ));
    }
}

fn ensure_initialized(timers: &ProcessorTimers) -> Result<(), super::Error> {
    if timers.initialized {
        Ok(())
    } else {
        Err(super::Error::TimerQueueNotInitialized)
    }
}

fn program_next(queue: &OwnedDeadlineQueue) -> Result<(), super::Error> {
    match queue.next_programming_deadline() {
        Some(deadline) => crate::hal::time::program_deadline(deadline)?,
        None => crate::hal::time::disable_local_timer(),
    }
    Ok(())
}

/// Serializes a callout's state, deadline membership, and ready membership.
/// Callout storage is permanently bound to this source CPU; the local IRQ mask
/// also prevents migration during a local comparator update.
pub(super) fn with_callout_queue<R>(
    cpu: CpuIndex,
    update: impl FnOnce(&mut OwnedDeadlineQueue, &mut super::callout::ReadyQueue) -> R,
) -> Result<R, super::Error> {
    let (result, prompt) = TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        let current = current_cpu()?;
        let before = timers.queue.next_deadline();
        let result = update(&mut timers.queue, &mut timers.ready);
        let prompt = if timers.queue.next_deadline() != before {
            if current == cpu {
                if let Err(error) = program_next(&timers.queue) {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: committed callout comparator update failed: {error:?}"
                    ));
                }
                false
            } else {
                // The queue mutation is durable before the remote prompt. IRQ
                // entry consumes this flag and observes the queue under its lock.
                REPROGRAM[cpu].store(true, Ordering::Release);
                true
            }
        } else {
            false
        };
        Ok::<_, super::Error>((result, prompt))
    })?;
    if prompt {
        crate::kernel::irq::reschedule::notify(cpu);
    }
    Ok(result)
}

/// Keeps the local mask until a condition waiter commits its scheduler park.
pub(super) unsafe fn with_callout_queue_mask_retained<R>(
    cpu: CpuIndex,
    inspect: impl FnOnce() -> R,
) -> (
    R,
    hyper::sync::InterruptMaskGuard<crate::hal::irq::LocalMask>,
) {
    // SAFETY: the caller consumes the returned mask in a prepared scheduler
    // transition or drops it before returning, as this API's contract requires.
    unsafe { TIMERS[cpu].with_mask_retained(|_| inspect()) }
}

/// Reprograms a source CPU after another CPU publishes an earlier deadline.
pub(super) fn service_reprogram_prompt() -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    if REPROGRAM[cpu].swap(false, Ordering::AcqRel) {
        TIMERS[cpu].with(|timers| {
            ensure_initialized(timers)?;
            program_next(&timers.queue)
        })?;
    }
    Ok(())
}

impl From<TimerQueueError> for super::Error {
    fn from(error: TimerQueueError) -> Self {
        Self::TimerQueue(error)
    }
}
