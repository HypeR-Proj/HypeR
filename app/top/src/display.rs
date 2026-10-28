// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Shared plain snapshot and terminal frame formatting.
use crate::{
    cli::Top,
    sampling::{Frame, ProcessSample, sort},
    view::View,
};
use hyper_text_ui::visible;

pub fn percent(value: u64, total: u64) -> String {
    let points = if total == 0 {
        0
    } else {
        u128::from(value) * 10_000 / u128::from(total)
    };
    format!("{}.{:02}%", points / 100, points % 100)
}
pub fn runtime(ns: u64) -> String {
    let centiseconds = ns / 10_000_000;
    format!(
        "{}:{:02}.{:02}",
        centiseconds / 6000,
        centiseconds / 100 % 60,
        centiseconds % 100
    )
}
fn safe(text: &str) -> String {
    visible(text, 0, usize::MAX)
}

pub fn summary(frame: &Frame, view: &View) -> Vec<String> {
    let cpu = frame.cpu;
    let total = frame.total_ns();
    let m = frame.memory;
    let mib = |bytes| bytes / (1024 * 1024);
    let uptime = cpu.captured_at_ns / 1_000_000_000;
    let running = frame
        .processes
        .iter()
        .filter(|p| p.state == "running")
        .count();
    vec![
        format!("top - {} CPUs", cpu.online_cpus),
        format!(
            "CPU: user-thread {} kthread {} vcpu {} idle {}",
            percent(cpu.user_thread_ns, total),
            percent(cpu.kernel_thread_ns, total),
            percent(cpu.vcpu_ns, total),
            percent(cpu.idle_ns, total)
        ),
        format!(
            "Mem MiB: {} total, {} allocated, {} free, {} reserved",
            mib(m.total_bytes),
            mib(m.total_bytes.saturating_sub(m.free_bytes)),
            mib(m.free_bytes),
            mib(m.total_bytes.saturating_sub(m.managed_bytes))
        ),
        format!(
            "Processes: {} total, {} running phase; Threads: {} total; cache: {} MiB",
            frame.processes.len(),
            running,
            frame.threads.len(),
            mib(m.file_cache_bytes)
        ),
        format!(
            "Up {}d {:02}:{:02}:{:02} | {} | task CPU: {} | delay {}s",
            uptime / 86400,
            uptime / 3600 % 24,
            uptime / 60 % 60,
            uptime % 60,
            if frame.since_boot {
                "since boot"
            } else {
                "interval"
            },
            if view.normalize {
                "whole machine"
            } else {
                "100% per CPU"
            },
            view.delay
        ),
    ]
}

pub fn rows(frame: &Frame, args: &Top, view: &View) -> Vec<ProcessSample> {
    let source = if view.threads {
        &frame.threads
    } else {
        &frame.processes
    };
    let mut rows: Vec<_> = source
        .iter()
        .filter(|row| {
            let matches = if view.threads {
                if row.owner == 0 {
                    args.filter.is_empty()
                } else {
                    args.filter.matches(row.owner, &row.owner_name)
                }
            } else {
                args.filter.matches(row.koid, &row.name)
            };
            matches && (!view.idle_hide || row.runtime_ns != 0)
        })
        .cloned()
        .collect();
    sort(&mut rows, view.sort);
    if view.reverse {
        rows.reverse();
    }
    if let Some(limit) = args.limit {
        rows.truncate(limit.get());
    }
    rows
}
pub fn heading(view: &View) -> &'static str {
    if view.threads {
        "TID (KOID)              %CPU        TIME+ ROLE      REGISTRY NAME / PROCESS [KOID]"
    } else {
        "PID (KOID)              %CPU        TIME+ THREADS STATE     NAME"
    }
}
pub fn row(row: &ProcessSample, frame: &Frame, view: &View) -> String {
    let denominator = if view.normalize {
        frame.total_ns()
    } else {
        frame.elapsed_ns
    };
    let cpu = percent(row.runtime_ns, denominator);
    let time = runtime(row.time_ns);
    if view.threads {
        let owner = if row.owner == 0 {
            "kernel".into()
        } else {
            format!("{} [0x{:016x}]", safe(&row.owner_name), row.owner)
        };
        format!(
            "0x{:016x} {:>9} {:>12} {:<9} {:<8} {} / {}",
            row.koid,
            cpu,
            time,
            row.role,
            row.state,
            safe(&row.name),
            owner
        )
    } else {
        format!(
            "0x{:016x} {:>9} {:>12} {:>7} {:<9} {}",
            row.koid,
            cpu,
            time,
            row.threads,
            row.state,
            safe(&row.name)
        )
    }
}
pub fn status(view: &View, shown: usize, count: usize) -> String {
    if let Some(prompt) = &view.prompt {
        return format!("Delay seconds (0.1..60; Esc cancels): {prompt}");
    }
    if !view.message.is_empty() {
        return view.message.clone();
    }
    format!(
        "{:?}{} | {} {}/{} | H threads I CPU | h help q quit",
        view.sort,
        if view.reverse { " reversed" } else { "" },
        if view.threads { "thread" } else { "process" },
        shown,
        count
    )
}
pub const HELP: &[&str] = &[
    "top - interactive help",
    "q / Ctrl-C   Quit; restore terminal settings",
    "Space/Enter  Sample and refresh immediately",
    "d / s        Set refresh delay (0.1..60 seconds)",
    "H            Toggle processes / individual threads",
    "I            Toggle 100% per CPU / whole-machine task CPU",
    "P / T        Sort by CPU usage / accumulated CPU time",
    "N / C        Sort by KOID / name",
    "R            Reverse sort order",
    "i            Toggle rows with zero sampled CPU activity",
    "Up/Down      Scroll rows; PageUp/PageDown scroll a page",
    "Left/Right   Scroll columns; Home resets; End shows last page",
    "",
    "First frame: since-boot counters; later frames: interval deltas.",
    "Process TIME+ retains observed retired-thread time during this run.",
    "STATE is process lifecycle; REGISTRY is thread residency, not R/S.",
    "RSS, per-CPU counters, load average and signals are not exposed.",
    "Any key returns; q quits.",
];

#[cfg(test)]
#[path = "../tests/display.rs"]
mod tests;
