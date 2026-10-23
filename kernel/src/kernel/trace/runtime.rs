// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::sync::InterruptMaskGuard;
use hyper::trace::{Error, Site, Ticket};

/// Installs the machine seam once. Capture remains disabled until explicitly
/// started; no tracing thread, timer or physical IRQ is installed.
pub fn initialize() -> Result<(), Error> {
    hyper::trace::install(record)
}

/// IRQ masking keeps CPU selection, timestamp and append on the same processor.
/// The try-only recorder also tolerates recursion without spinning on its owner.
fn record(ticket: Ticket, site: &'static Site, fields: &mut dyn FnMut() -> [u64; 4]) {
    // SAFETY: This bounded nonblocking scope never yields or migrates. The guard
    // restores the caller's prior mask on this CPU, including nested IRQ callers.
    let _mask = unsafe { InterruptMaskGuard::<crate::hal::irq::LocalMask>::acquire() };
    if let Some(cpu) = crate::kernel::cpu::current_index() {
        hyper::trace::record_at(
            ticket,
            cpu.get(),
            site,
            || crate::kernel::time::monotonic_nanoseconds().ok(),
            fields,
        );
    }
}
