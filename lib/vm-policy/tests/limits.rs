// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{
    BUSINESS_VM_CAPACITY, FLEET_CONTROL_PLANE_HEADROOM, VM_FLEET_LIMITS, VM_INSTANCE_LIMITS,
};

#[test]
fn fleet_domain_contains_business_slots_and_control_plane() {
    let fleet = VM_FLEET_LIMITS;
    let instance = VM_INSTANCE_LIMITS;
    let headroom = FLEET_CONTROL_PLANE_HEADROOM;
    assert_eq!(
        BUSINESS_VM_CAPACITY as usize + 1,
        hyper_os::vm::IO_MAX_CLIENTS
    );
    assert_eq!(
        fleet.kernel_memory_bytes,
        BUSINESS_VM_CAPACITY * instance.kernel_memory_bytes + headroom.kernel_memory_bytes
    );
    assert_eq!(
        fleet.processes,
        BUSINESS_VM_CAPACITY * instance.processes + headroom.processes
    );
    assert_eq!(
        fleet.threads,
        BUSINESS_VM_CAPACITY * instance.threads + headroom.threads
    );
    assert_eq!(
        fleet.handles,
        BUSINESS_VM_CAPACITY * instance.handles + headroom.handles
    );
    assert_eq!(
        fleet.kernel_objects,
        BUSINESS_VM_CAPACITY * instance.kernel_objects + headroom.kernel_objects
    );
    assert_eq!(
        fleet.committed_pages,
        BUSINESS_VM_CAPACITY * instance.committed_pages + headroom.committed_pages
    );
    assert_eq!(
        fleet.pinned_pages,
        BUSINESS_VM_CAPACITY * instance.pinned_pages + headroom.pinned_pages
    );
    assert_eq!(
        fleet.guest_pages,
        BUSINESS_VM_CAPACITY * instance.guest_pages + headroom.guest_pages
    );
    assert_eq!(
        fleet.ipc_messages,
        BUSINESS_VM_CAPACITY * instance.ipc_messages + headroom.ipc_messages
    );
    assert_eq!(
        fleet.ipc_bytes,
        BUSINESS_VM_CAPACITY * instance.ipc_bytes + headroom.ipc_bytes
    );
    assert_eq!(
        fleet.ipc_handles,
        BUSINESS_VM_CAPACITY * instance.ipc_handles + headroom.ipc_handles
    );
    assert_eq!(
        fleet.subscriptions,
        BUSINESS_VM_CAPACITY * instance.subscriptions + headroom.subscriptions
    );
    assert_eq!(
        fleet.timers,
        BUSINESS_VM_CAPACITY * instance.timers + headroom.timers
    );
    assert_eq!(
        fleet.virtual_machines,
        BUSINESS_VM_CAPACITY * instance.virtual_machines + headroom.virtual_machines
    );
    assert_eq!(
        fleet.virtual_cpus,
        BUSINESS_VM_CAPACITY * instance.virtual_cpus + headroom.virtual_cpus
    );
    assert_eq!(
        fleet.device_leases,
        BUSINESS_VM_CAPACITY * instance.device_leases + headroom.device_leases
    );
    assert_eq!(
        fleet.dma_mappings,
        BUSINESS_VM_CAPACITY * instance.dma_mappings + headroom.dma_mappings
    );
    assert_eq!(
        fleet.user_address_spaces,
        BUSINESS_VM_CAPACITY * instance.user_address_spaces + headroom.user_address_spaces
    );
    assert_eq!(
        fleet.user_mappings,
        BUSINESS_VM_CAPACITY * instance.user_mappings + headroom.user_mappings
    );
}

#[test]
fn business_guest_budget_excludes_the_resident_io_vm() {
    assert_eq!(VM_INSTANCE_LIMITS.guest_pages, 64 * 1024);
    assert_eq!(VM_INSTANCE_LIMITS.virtual_machines, 1);
    assert_eq!(VM_FLEET_LIMITS.virtual_machines, BUSINESS_VM_CAPACITY);
}
