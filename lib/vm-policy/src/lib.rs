// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Resource-containment policy for Native VM services and instances.

pub mod affinity;
pub mod fleet;
pub mod image;

use hyper_os::task::ResourceLimits;

/// Limit installed on each disposable VM runtime domain.
pub const VM_INSTANCE_LIMITS: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: 64 * 1024 * 1024,
    processes: 8,
    threads: 16,
    handles: 256,
    kernel_objects: 1024,
    committed_pages: 72 * 1024,
    pinned_pages: 65 * 1024,
    guest_pages: 64 * 1024,
    ipc_messages: 128,
    ipc_bytes: 1024 * 1024,
    ipc_handles: 64,
    subscriptions: 128,
    timers: 128,
    virtual_machines: 1,
    // Both Arm GIC backends expose up to eight configured guest CPUs.
    virtual_cpus: 8,
    device_leases: 8,
    dma_mappings: 64,
    user_address_spaces: 8,
    user_mappings: 256,
};

const FLEET_CONTROL_PLANE_HEADROOM: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: 64 * 1024 * 1024,
    processes: 4,
    threads: 8,
    handles: 256,
    kernel_objects: 1024,
    committed_pages: 8 * 1024,
    pinned_pages: 1024,
    guest_pages: 0,
    ipc_messages: 128,
    ipc_bytes: 1024 * 1024,
    ipc_handles: 64,
    subscriptions: 128,
    timers: 128,
    virtual_machines: 0,
    virtual_cpus: 0,
    device_leases: 8,
    dma_mappings: 64,
    user_address_spaces: 4,
    user_mappings: 256,
};

/// Same bound as the Native I/O connection table and board configuration.
/// Resource limits bound charges; they do not reserve physical RAM in advance.
pub const BUSINESS_VM_CAPACITY: u64 = (hyper_os::vm::IO_MAX_CLIENTS - 1) as u64;

/// Aggregate ancestor budget for business VM slots and manager headroom.
/// The resident I/O VM stays in the core-services domain. Each business runtime
/// receives the unchanged `VM_INSTANCE_LIMITS` child domain; delegated domain creation cannot
/// exceed this finite ancestor budget.
pub const VM_FLEET_LIMITS: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.kernel_memory_bytes
        + FLEET_CONTROL_PLANE_HEADROOM.kernel_memory_bytes,
    processes: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.processes
        + FLEET_CONTROL_PLANE_HEADROOM.processes,
    threads: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.threads
        + FLEET_CONTROL_PLANE_HEADROOM.threads,
    handles: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.handles
        + FLEET_CONTROL_PLANE_HEADROOM.handles,
    kernel_objects: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.kernel_objects
        + FLEET_CONTROL_PLANE_HEADROOM.kernel_objects,
    committed_pages: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.committed_pages
        + FLEET_CONTROL_PLANE_HEADROOM.committed_pages,
    pinned_pages: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.pinned_pages
        + FLEET_CONTROL_PLANE_HEADROOM.pinned_pages,
    guest_pages: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.guest_pages
        + FLEET_CONTROL_PLANE_HEADROOM.guest_pages,
    ipc_messages: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.ipc_messages
        + FLEET_CONTROL_PLANE_HEADROOM.ipc_messages,
    ipc_bytes: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.ipc_bytes
        + FLEET_CONTROL_PLANE_HEADROOM.ipc_bytes,
    ipc_handles: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.ipc_handles
        + FLEET_CONTROL_PLANE_HEADROOM.ipc_handles,
    subscriptions: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.subscriptions
        + FLEET_CONTROL_PLANE_HEADROOM.subscriptions,
    timers: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.timers + FLEET_CONTROL_PLANE_HEADROOM.timers,
    virtual_machines: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.virtual_machines
        + FLEET_CONTROL_PLANE_HEADROOM.virtual_machines,
    virtual_cpus: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.virtual_cpus
        + FLEET_CONTROL_PLANE_HEADROOM.virtual_cpus,
    device_leases: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.device_leases
        + FLEET_CONTROL_PLANE_HEADROOM.device_leases,
    dma_mappings: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.dma_mappings
        + FLEET_CONTROL_PLANE_HEADROOM.dma_mappings,
    user_address_spaces: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.user_address_spaces
        + FLEET_CONTROL_PLANE_HEADROOM.user_address_spaces,
    user_mappings: BUSINESS_VM_CAPACITY * VM_INSTANCE_LIMITS.user_mappings
        + FLEET_CONTROL_PLANE_HEADROOM.user_mappings,
};

#[cfg(test)]
#[path = "../tests/limits.rs"]
mod tests;
