// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[path = "../../../../src/kernel/vm/reconcile.rs"]
mod reconcile_model;

use std::sync::{Arc, Barrier};

use reconcile_model::ReconcilePublication;

#[test]
fn publication_coalesces_and_restoration_reopens_work() {
    let publication = ReconcilePublication::new();
    assert!(!publication.pending());

    publication.publish();
    publication.publish();
    assert!(publication.take());
    assert!(!publication.take());

    publication.restore();
    assert!(publication.pending());
    assert!(publication.take());
}

#[test]
fn a_racing_publish_is_claimed_or_remains_pending() {
    const ITERATIONS: usize = 256;

    for _ in 0..ITERATIONS {
        let publication = Arc::new(ReconcilePublication::new());
        let start = Arc::new(Barrier::new(2));
        let producer_publication = publication.clone();
        let producer_start = start.clone();
        let producer = std::thread::spawn(move || {
            producer_start.wait();
            producer_publication.publish();
        });

        start.wait();
        let claimed = publication.take();
        assert!(producer.join().is_ok());
        assert!(claimed || publication.pending());
    }
}

#[test]
fn pre_entry_refresh_preserves_a_quiesce_request_until_bank_detachment() {
    use hyper::vm::arm::gic::quiesce::ActiveAccessQuiesce;
    use hyper::vm::arm::gic::{
        GicInterruptId, InterruptGroup, InterruptTrigger, VirtualGicBuilder,
    };
    use hyper::vm::interrupt::VirtualCpuId;

    // Exercise both sides of the final pre-entry observation. This uses the
    // production publication and quiesce state machines, without relying on
    // a periodic tick or an unrelated scheduling event to release residency.
    for publish_before_refresh in [true, false] {
        let entering = VirtualCpuId::new(0);
        let requester = VirtualCpuId::new(1);
        let irq = crate::require_some(GicInterruptId::new(48));
        let mut builder = crate::require_ok(VirtualGicBuilder::new(2));
        crate::require_ok(builder.configure(
            irq,
            entering,
            0x80,
            InterruptGroup::Group1,
            InterruptTrigger::Level,
        ));
        let mut controller = crate::require_ok(builder.finish(1));
        let mut gate = crate::require_ok(ActiveAccessQuiesce::try_new(2));
        let publication = ReconcilePublication::new();
        let mut slots = [None];

        // activate() consumes older work before try_enter(), then reserves
        // residency and loads its initial bank with local IRQs masked.
        assert!(!publication.take());
        assert!(gate.try_enter(entering));
        assert!(gate.try_enter(requester));
        crate::require_ok(controller.refill_current(entering, &mut slots));

        crate::require_ok(gate.begin_deactivate(requester, irq.get(), None, true));
        gate.bank_saved(requester, &mut controller);
        assert_eq!(gate.take_prompts(), 3);
        if publish_before_refresh {
            publication.publish();
        }
        // A final refresh is not a detach acknowledgement. Using take()
        // here would consume the only reason for the pending SGI's IRQ tail
        // to detach entering, leaving requester blocked behind its residency.
        if publication.pending() {
            crate::require_ok(controller.refill_current(entering, &mut slots));
        }
        if !publish_before_refresh {
            publication.publish();
        }
        assert!(gate.gate_closed());
        assert!(gate.take(requester).is_none());
        assert!(
            publication.pending(),
            "IRQ tail must still detach this bank"
        );

        crate::require_ok(controller.synchronize(entering, &slots));
        gate.bank_saved(entering, &mut controller);
        assert!(!gate.gate_closed());
        assert_eq!(gate.take(requester), Some(Ok(None)));
        // The following activation may consume the publication, because it
        // starts detached and rechecks admission before reserving hardware.
        assert!(publication.take());
        assert!(gate.try_enter(entering));
        gate.bank_saved(entering, &mut controller);
    }
}
