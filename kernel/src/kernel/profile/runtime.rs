// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{DEPTH, INFO_SIZE, Result, SAMPLE_SIZE};
use crate::hal::pmu::{self, SampleContext};
use core::ptr::addr_of;
use hyper::abi::native::*;
use hyper::cpu::{CpuIndex, MAX_CPUS, PerCpu};
use hyper::drivers::platform::PlatformDevice;
use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger};
use hyper::sync::{
    InterruptSpinLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

const CAPACITY: usize = 256;
type Lock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;
#[derive(Clone, Copy)]
struct Sample([u64; 24]);
struct Buffer {
    samples: [Sample; CAPACITY],
    count: u64,
}
impl Buffer {
    const fn new() -> Self {
        Self {
            samples: [Sample([0; 24]); CAPACITY],
            count: 0,
        }
    }
}
struct Config {
    generation: u64,
    period: u64,
    deadline: u64,
}
static CONTROL: Lock<Config> = Lock::new(Config {
    generation: 0,
    period: 0,
    deadline: 0,
});
static BUFFERS: PerCpu<Lock<Buffer>> = PerCpu::new([const { Lock::new(Buffer::new()) }; MAX_CPUS]);
static ADMISSION: super::admission::Admission<MAX_CPUS> = super::admission::Admission::new();
static PERIOD: AtomicU64 = AtomicU64::new(0);
static DEADLINE: AtomicU64 = AtomicU64::new(0);
static ROUTES: PerCpu<AtomicU64> = PerCpu::new([const { AtomicU64::new(0) }; MAX_CPUS]);
static PROBED: PerCpu<AtomicBool> = PerCpu::new([const { AtomicBool::new(false) }; MAX_CPUS]);
static SUPPORTED: PerCpu<AtomicBool> = PerCpu::new([const { AtomicBool::new(false) }; MAX_CPUS]);
static DROPPED: PerCpu<AtomicU64> = PerCpu::new([const { AtomicU64::new(0) }; MAX_CPUS]);
static READY: AtomicBool = AtomicBool::new(false);
unsafe extern "C" {
    static __image_start: u8;
    static __rodata_start: u8;
}

pub(crate) fn owns_node(node: &PlatformDevice) -> bool {
    node.is_compatible("arm,armv8-pmuv3")
        || node.is_compatible("arm,cortex-a76-pmu")
        || node.is_compatible("arm,cortex-a57-pmu")
        || node.is_compatible("arm,cortex-a72-pmu")
}

/// Discovers the firmware PPI or per-CPU SPI routes. No guessed interrupt numbers.
/// Partial failure keeps the observer quiescing PMUs and any installed routes idle;
/// profiling remains unavailable.
pub(crate) fn initialize(
    nodes: &[PlatformDevice],
    domain: crate::kernel::irq::interrupt::IrqDomainId,
) {
    if let Err(error) = install(nodes, domain) {
        crate::pr_warn!("HypeR: PMU profiling unavailable: {error}");
    }
}
fn install(
    nodes: &[PlatformDevice],
    domain: crate::kernel::irq::interrupt::IrqDomainId,
) -> core::result::Result<(), &'static str> {
    use crate::kernel::irq::interrupt;
    let mut pmus = nodes.iter().filter(|node| owns_node(node));
    let node = pmus.next().ok_or("no supported firmware PMU")?;
    if pmus.next().is_some() {
        return Err("multiple PMU nodes are unsupported");
    }
    let cells = node.interrupt_cells();
    let n = crate::kernel::cpu::online_cpu_count();
    if n == 0 || n > 64 {
        return Err("more than 64 CPUs");
    }
    let mut routes = [0u32; MAX_CPUS];
    if cells.len() == 3 && cells[0] == 1 {
        let route = crate::hal::irq::decode_platform(cells).map_err(|_| "invalid PMU PPI")?;
        if route.trigger != hyper::platform::PlatformInterruptTrigger::Level {
            return Err("PMU requires level interrupts");
        }
        routes[..n].fill(route.interrupt);
    } else {
        let affinity = node
            .property("interrupt-affinity")
            .ok_or("missing SPI affinity")?;
        if cells.len() % 3 != 0 || affinity.len() != cells.len() / 3 * 4 {
            return Err("invalid SPI affinity");
        }
        let boot = crate::hal::cpu::current_hardware_id().get();
        let topology = crate::kernel::boot::with_boot_state(|state| state.platform.cpus);
        let mut ids = [0u64; MAX_CPUS];
        ids[0] = boot;
        let mut next = 1;
        for cpu in topology
            .as_slice()
            .iter()
            .filter(|cpu| cpu.hardware_id != boot)
        {
            if next < n {
                ids[next] = cpu.hardware_id;
                next += 1;
            }
        }
        for (spec, phandle) in cells.chunks_exact(3).zip(affinity.chunks_exact(4)) {
            if spec[0] != 0 {
                return Err("mixed PMU interrupt types");
            }
            let cpu_node = nodes
                .iter()
                .find(|candidate| candidate.property("phandle") == Some(phandle))
                .ok_or("unknown CPU phandle")?;
            let reg = cpu_node.property("reg").ok_or("missing CPU reg")?;
            if !matches!(reg.len(), 4 | 8) {
                return Err("invalid CPU reg");
            }
            let hardware = reg
                .iter()
                .fold(0u64, |value, byte| (value << 8) | u64::from(*byte));
            let Some(index) = ids[..n].iter().position(|id| *id == hardware) else {
                continue;
            };
            let route = crate::hal::irq::decode_platform(spec).map_err(|_| "invalid PMU SPI")?;
            if route.trigger != hyper::platform::PlatformInterruptTrigger::Level {
                return Err("PMU requires level interrupts");
            }
            if routes[index] != 0 {
                return Err("duplicate PMU CPU");
            }
            routes[index] = route.interrupt;
        }
        if routes[..n].contains(&0) {
            return Err("incomplete PMU CPU routes");
        }
    }
    for (index, &id) in routes[..n].iter().enumerate() {
        if id >= 32 && routes[..index].contains(&id) {
            return Err("duplicate PMU SPI");
        }
    }
    // Publish the quiescing observer before activating any controller route.
    // If registration fails partway, timer IRQs still disable each local PMU;
    // no capture can start because READY remains false.
    for (index, &id) in routes[..n].iter().enumerate() {
        let cpu = CpuIndex::new(index).ok_or("invalid CPU")?;
        ROUTES[cpu].store(u64::from(id), Ordering::Release);
    }
    pmu::install(observe).map_err(|_| "PMU observer already installed")?;
    for (index, &id) in routes[..n].iter().enumerate() {
        if index == 0 || id >= 32 {
            let prepared = domain
                .prepare_shared_mapping(
                    InterruptId::new(id),
                    InterruptPriority::Low,
                    InterruptTrigger::Level,
                    0,
                    |_, _| interrupt::HandlerResult::Handled,
                )
                .map_err(|_| "PMU IRQ registration")?;
            if id >= 32 {
                let cpu = CpuIndex::new(index).ok_or("invalid CPU")?;
                if interrupt::route_prepared_to_cpu(&prepared, cpu).is_err() {
                    if let Err(failure) = interrupt::discard_prepared(prepared) {
                        let (_, retained) = failure.into_parts();
                        core::mem::forget(retained);
                    }
                    return Err("PMU SPI routing");
                }
            }
            match interrupt::activate(prepared) {
                Ok(registration) => registration.retain_permanently(),
                Err(failure) => {
                    let (_, retained) = failure.into_parts();
                    core::mem::forget(retained);
                    return Err("PMU IRQ activation");
                }
            }
        }
    }
    READY.store(true, Ordering::Release);
    crate::pr_info!("HypeR: PMUv3 profiler ready on {n} CPUs (disabled until requested)");
    Ok(())
}

pub(crate) fn start(period: u64, seconds: u64) -> Result<u64> {
    if !(1_000_000..=1_000_000_000).contains(&period) || !(1..=300).contains(&seconds) {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    CONTROL.with(|config| {
        if !READY.load(Ordering::Acquire)
            || SUPPORTED
                .iter()
                .take(crate::kernel::cpu::online_cpu_count())
                .any(|v| !v.load(Ordering::Acquire))
        {
            return Err(HYPER_NATIVE_STATUS_NOT_SUPPORTED);
        }
        if ADMISSION.requested() != 0 || !ADMISSION.quiescent() {
            return Err(HYPER_NATIVE_STATUS_BUSY);
        }
        let generation = config
            .generation
            .checked_add(1)
            .filter(|generation| *generation <= super::admission::MAX_GENERATION)
            .ok_or(HYPER_NATIVE_STATUS_RESOURCE_LIMIT)?;
        let now = crate::kernel::time::monotonic_nanoseconds()
            .map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)?;
        let deadline = now
            .checked_add(seconds * 1_000_000_000)
            .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        for buffer in BUFFERS.iter() {
            buffer.with(|buffer| buffer.count = 0);
        }
        for dropped in DROPPED.iter() {
            dropped.store(0, Ordering::Relaxed);
        }
        *config = Config {
            generation,
            period,
            deadline,
        };
        PERIOD.store(period, Ordering::Relaxed);
        DEADLINE.store(deadline, Ordering::Relaxed);
        ADMISSION.publish(generation);
        Ok(generation)
    })
}

/// Disarms asynchronously on each CPU's next IRQ. BUSY means stop is requested
/// but one CPU has not yet acknowledged; callers retry, never spin in the kernel.
pub(crate) fn stop(generation: u64) -> Result<()> {
    CONTROL.with(|config| {
        if generation == 0 || generation != config.generation {
            return Err(HYPER_NATIVE_STATUS_BAD_STATE);
        }
        ADMISSION.close(generation);
        if !ADMISSION.quiescent() {
            Err(HYPER_NATIVE_STATUS_BUSY)
        } else {
            Ok(())
        }
    })
}

/// Info uses `cpu=u64::MAX`; otherwise read the first retained sample at/after
/// cursor. Reading is non-destructive and includes its sequence to expose gaps.
pub(crate) fn query(generation: u64, cpu: u64, cursor: u64, out: &mut [u8]) -> Result<usize> {
    CONTROL.with(|config| {
        if !READY.load(Ordering::Acquire) {
            return Err(HYPER_NATIVE_STATUS_NOT_SUPPORTED);
        }
        if cpu == u64::MAX {
            if generation != 0 || cursor != 0 || out.len() < INFO_SIZE {
                return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
            }
            let armed = (0..crate::kernel::cpu::online_cpu_count()).fold(0, |mask, cpu| {
                mask | (u64::from(ADMISSION.owner(cpu) != 0) << cpu)
            });
            encode(
                &[
                    config.generation,
                    config.period,
                    config.deadline,
                    crate::kernel::cpu::online_cpu_count() as u64,
                    ADMISSION.requested(),
                    armed,
                    CAPACITY as u64,
                    DEPTH as u64,
                    ADMISSION.active(),
                    DROPPED
                        .iter()
                        .map(|count| count.load(Ordering::Relaxed))
                        .sum(),
                ],
                out,
            );
            out[80..INFO_SIZE].fill(0);
            let revision = concat!(
                env!("HYPER_BUILD_REVISION"),
                " ",
                env!("HYPER_BUILD_TIMESTAMP")
            )
            .as_bytes();
            out[80..80 + revision.len().min(64)]
                .copy_from_slice(&revision[..revision.len().min(64)]);
            return Ok(INFO_SIZE);
        }
        if generation == 0 || generation != config.generation {
            return Err(HYPER_NATIVE_STATUS_BAD_STATE);
        }
        let cpu = usize::try_from(cpu)
            .ok()
            .filter(|cpu| *cpu < crate::kernel::cpu::online_cpu_count())
            .and_then(CpuIndex::new)
            .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
        if out.len() < SAMPLE_SIZE {
            return Err(HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL);
        }
        BUFFERS[cpu].with(|buffer| {
            let sequence = cursor.max(buffer.count.saturating_sub(CAPACITY as u64));
            if sequence >= buffer.count {
                return Err(HYPER_NATIVE_STATUS_WOULD_BLOCK);
            }
            encode(&buffer.samples[sequence as usize % CAPACITY].0, out);
            Ok(SAMPLE_SIZE)
        })
    })
}
fn encode(words: &[u64], bytes: &mut [u8]) {
    for (word, dst) in words.iter().zip(bytes.chunks_exact_mut(8)) {
        dst.copy_from_slice(&word.to_le_bytes());
    }
}

fn observe(interrupt: InterruptId, context: SampleContext) {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        return;
    };
    if ROUTES[cpu].load(Ordering::Acquire) == 0 {
        return;
    }
    if !PROBED[cpu].swap(true, Ordering::Relaxed) {
        let available = pmu::available();
        if available {
            pmu::initialize();
        }
        SUPPORTED[cpu].store(available, Ordering::Release);
    }
    if !SUPPORTED[cpu].load(Ordering::Relaxed) {
        return;
    }
    let request = ADMISSION.requested();
    if request == 0 {
        if ADMISSION.owner(cpu.get()) != 0
            || u64::from(interrupt.get()) == ROUTES[cpu].load(Ordering::Relaxed)
        {
            pmu::stop();
            if ADMISSION.owner(cpu.get()) != 0 {
                ADMISSION.release(cpu.get());
            }
        }
        return;
    }
    let Ok(now) = crate::kernel::time::monotonic_nanoseconds() else {
        pmu::stop();
        if ADMISSION.owner(cpu.get()) != 0 {
            ADMISSION.release(cpu.get());
        }
        ADMISSION.close(request);
        return;
    };
    if now >= DEADLINE.load(Ordering::Relaxed) {
        if request != 0 {
            ADMISSION.close(request);
        }
        if ADMISSION.owner(cpu.get()) != 0 {
            pmu::stop();
            ADMISSION.release(cpu.get());
        }
        return;
    }
    let period = PERIOD.load(Ordering::Relaxed);
    if ADMISSION.owner(cpu.get()) != request {
        // Claim the packed generation before arming; control cannot recycle
        // the buffer until this CPU disarms and releases its admitted writer.
        if !ADMISSION.claim(cpu.get(), request) {
            return;
        }
        pmu::start(period);
        return;
    }
    if u64::from(interrupt.get()) != ROUTES[cpu].load(Ordering::Relaxed) || !pmu::overflowed() {
        return;
    }
    pmu::stop();
    if BUFFERS[cpu]
        .try_with(|buffer| {
            let mut sample = Sample([0; 24]);
            sample.0[0] = buffer.count;
            sample.0[1] = now;
            sample.0[2] = cpu.get() as u64;
            sample.0[3] = context.world;
            sample.0[6] = DROPPED[cpu].load(Ordering::Relaxed);
            sample.0[7] = period;
            if context.world == 0 {
                unwind(cpu, context, &mut sample);
            }
            buffer.samples[buffer.count as usize % CAPACITY] = sample;
            buffer.count = buffer.count.saturating_add(1);
        })
        .is_none()
    {
        DROPPED[cpu].fetch_add(1, Ordering::Relaxed);
    }
    if ADMISSION.requested() == request {
        pmu::reload(period);
    } else {
        ADMISSION.release(cpu.get());
    }
}

fn unwind(cpu: CpuIndex, context: SampleContext, sample: &mut Sample) {
    let base = addr_of!(__image_start) as usize;
    let end = addr_of!(__rodata_start) as usize;
    let offset = |pc: usize| (base..end).contains(&pc).then(|| (pc - base) as u64);
    let Some(pc) = offset(context.pc) else {
        sample.0[5] = 1;
        return;
    };
    sample.0[8] = pc;
    sample.0[4] = 1;
    let task = crate::kernel::task::scheduler::crash_snapshot(cpu.get());
    let bounds = task
        .and_then(|task| task.stack)
        .filter(|(bottom, top)| *bottom <= context.sp && context.sp < *top)
        .or_else(|| crate::hal::exception::bootstrap_stack_bounds(context.sp as u64))
        .or_else(|| crate::kernel::mm::stack::exception_stack_bounds(cpu.get(), context.sp));
    let Some((bottom, top)) = bounds else {
        sample.0[5] = 1;
        return;
    };
    let mut fp = context.fp;
    for depth in 1..DEPTH {
        // SAFETY: The local CPU is interrupted with IRQs masked, so its current
        // pinned kernel stack cannot execute or retire. Bounds reject user and
        // guest frames; HAL validates alignment and the entire frame record.
        let record = unsafe {
            crate::hal::exception::CrashContext::previous_stack_frame(
                fp,
                bottom.max(context.sp),
                top,
            )
        };
        let (previous, link) = match record {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(_) => {
                sample.0[5] = 1;
                return;
            }
        };
        let Some(pc) = link.checked_sub(4).and_then(offset) else {
            return;
        };
        sample.0[8 + depth] = pc;
        sample.0[4] += 1;
        if previous <= fp {
            return;
        }
        fp = previous;
    }
    sample.0[5] = 1;
}
