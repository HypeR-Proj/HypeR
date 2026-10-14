// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_vm_policy::fleet::{self, Response};
use std::io::Write;

pub(crate) fn write_response(
    output: &mut impl Write,
    response: Response,
) -> Result<(), Box<dyn std::error::Error>> {
    match response {
        Response::AffinityAccepted { vcpu } => writeln!(
            output,
            "vCPU {vcpu}: affinity accepted; inspect vmm status for placement"
        )?,
        Response::Accepted => writeln!(output, "accepted")?,
        Response::Error { message } => return Err(std::io::Error::other(message).into()),
        Response::Entries { mut machines } => {
            machines.sort_by(|a, b| a.name.cmp(&b.name));
            writeln!(
                output,
                "NAME                              STATE      AUTOSTART  ACCESS      IMAGE"
            )?;
            if machines.is_empty() {
                writeln!(
                    output,
                    "(no virtual machines; use 'vmm create NAME --config PATH')"
                )?;
            }
            for machine in machines {
                writeln!(
                    output,
                    "{:<32}  {:<11}  {:<9}  {:<10}  {}",
                    machine.name,
                    machine.state,
                    if machine.autostart { "yes" } else { "no" },
                    if machine.read_only {
                        "read-only"
                    } else {
                        "managed"
                    },
                    machine.image
                )?;
                if let Some(network) = &machine.network {
                    writeln!(
                        output,
                        "  network: {}; MAC: {}; I/O client: {}",
                        network.network, network.mac, network.client
                    )?;
                }
                for placement in &machine.placement {
                    writeln!(
                        output,
                        "  vCPU {}: pCPU {}",
                        placement.vcpu,
                        placement
                            .host_cpu
                            .map_or_else(|| "unassigned".into(), |cpu| cpu.to_string())
                    )?;
                }
                if let (Some(vcpus), Some(bytes)) = (machine.vcpus, machine.memory_bytes) {
                    writeln!(
                        output,
                        "  vCPUs: {vcpus}; RAM capacity: {} MiB",
                        bytes / (1024 * 1024)
                    )?;
                    if let Some(resident) = machine.resident_memory_bytes {
                        writeln!(
                            output,
                            "  allocated VM backing: {resident} bytes ({} KiB)",
                            resident / 1024
                        )?;
                    } else {
                        writeln!(output, "  allocated VM backing: unavailable")?;
                    }
                } else if !matches!(machine.state, fleet::State::Stopped | fleet::State::Failed) {
                    writeln!(output, "  VM metrics: unavailable")?;
                }
                if let Some(metrics) = machine.guest_metrics {
                    write_guest_metrics(output, metrics)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/output.rs"]
mod tests;

fn write_guest_metrics(
    output: &mut impl Write,
    metrics: hyper_service::io::metrics::GuestMetrics,
) -> std::io::Result<()> {
    use hyper_service::io::metrics::AgentState;
    if !metrics.valid() {
        return Err(std::io::Error::other("invalid guest usage reply"));
    }
    match metrics.state {
        AgentState::Unavailable => {
            return writeln!(
                output,
                "  guest usage: unavailable (agent disconnected or timed out)"
            );
        }
        AgentState::Unsupported => {
            return writeln!(
                output,
                "  guest usage: unavailable (agent metrics unsupported)"
            );
        }
        AgentState::Stale => {
            return writeln!(
                output,
                "  guest usage: unavailable (stale sample, {} ms old)",
                metrics.sample_age_ms.unwrap_or(0)
            );
        }
        AgentState::Sampling | AgentState::Ready => (),
    }
    if let Some(cpu) = metrics.cpu {
        writeln!(
            output,
            "  guest CPU: {}.{:02}% busy; {}.{:02}% I/O wait ({} ms interval, all vCPUs)",
            cpu.busy_basis_points / 100,
            cpu.busy_basis_points % 100,
            cpu.iowait_basis_points / 100,
            cpu.iowait_basis_points % 100,
            cpu.interval_ms
        )?;
    } else {
        writeln!(output, "  guest CPU: sampling")?;
    }
    if let Some(memory) = metrics.memory {
        writeln!(
            output,
            "  guest memory: {} KiB used / {} KiB total; {} KiB available; {} KiB free",
            (memory.total_bytes - memory.available_bytes) / 1024,
            memory.total_bytes / 1024,
            memory.available_bytes / 1024,
            memory.free_bytes / 1024
        )?;
        writeln!(
            output,
            "  guest cache: {} KiB buffers; {} KiB cached; {} KiB reclaimable slab; sample age {} ms",
            memory.buffers_bytes / 1024,
            memory.cached_bytes / 1024,
            memory.reclaimable_slab_bytes / 1024,
            metrics.sample_age_ms.unwrap_or(0)
        )?;
    }
    Ok(())
}
