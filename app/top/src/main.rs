// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped monitor with immediate snapshots and deadline-driven input.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;
#[cfg(target_os = "hyper")]
extern crate hyper_text_ui_shared as hyper_text_ui;

use clap::Parser;
use hyper_os::inspect::{CpuInspector, MemoryInspector, ScanCursor, TaskInspector};
use hyper_os::startup;
use hyper_text_ui::{Error, Screen, Session};
use hyper_top::{
    cli::Top,
    display,
    sampling::{Frame, ProcessSample, SampleSet, Sampler, Snapshot, ThreadSample},
    view::{Action, View},
};
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

fn capture(
    tasks: &TaskInspector,
    cpu: &CpuInspector,
    memory: &MemoryInspector,
) -> hyper_os::Result<Snapshot> {
    let cpu = cpu.read()?;
    let memory = memory.read()?;
    let mut processes = Vec::new();
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_processes(position)?;
        for process in page.entries() {
            processes.push(ProcessSample {
                koid: process.koid.get(),
                name: process.name.as_str().into(),
                state: process.phase.name(),
                ..Default::default()
            });
        }
        cursor = page.next();
    }
    let mut threads = SampleSet::new();
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = tasks.scan_threads(position)?;
        for thread in page.entries() {
            threads.insert(
                thread.koid.get(),
                ThreadSample {
                    process_koid: thread.process_koid.map_or(0, |koid| koid.get()),
                    thread_koid: thread.koid.get(),
                    runtime_ns: thread.runtime_ns,
                    name: thread.name.as_str().into(),
                    role: thread.role.name(),
                    state: thread.registry_phase.name(),
                },
            );
        }
        cursor = page.next();
    }
    Ok(Snapshot {
        cpu,
        memory,
        processes,
        threads,
    })
}

fn render(
    frame: &Frame,
    args: &Top,
    view: &mut View,
    session: Option<&Session>,
) -> Result<(), Error> {
    let summary = display::summary(frame, view);
    let rows = display::rows(frame, args, view);
    if let Some(session) = session {
        let mut screen = Screen::new(session.columns);
        let height = session.rows.saturating_sub(summary.len() + 2).max(1);
        view.offset = view.offset.min(rows.len().saturating_sub(height));
        for y in 0..session.rows - 1 {
            if view.help {
                screen.row(y, display::HELP.get(y).copied().unwrap_or(""), 0, false);
            } else if let Some(line) = summary.get(y) {
                screen.row(y, line, 0, false);
            } else if y == summary.len() {
                screen.row(y, display::heading(view), view.left, true);
            } else {
                let index = view.offset + y - summary.len() - 1;
                let line = rows
                    .get(index)
                    .map_or(String::new(), |row| display::row(row, frame, view));
                screen.row(y, &line, view.left, false);
            }
        }
        screen.row(
            session.rows - 1,
            &display::status(
                view,
                view.offset.saturating_add(height).min(rows.len()),
                rows.len(),
            ),
            0,
            true,
        );
        session.write(screen.bytes())?;
    } else {
        let mut output = std::io::stdout().lock();
        for line in summary {
            writeln!(output, "{line}")?;
        }
        writeln!(output, "{}", display::heading(view))?;
        for row in rows {
            writeln!(output, "{}", display::row(&row, frame, view))?;
        }
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}
fn now() -> hyper_os::Result<u64> {
    Ok(hyper_os::time::monotonic_now()?.as_nanoseconds())
}
fn deadline(frame: &Frame, view: &View) -> u64 {
    frame
        .cpu
        .captured_at_ns
        .saturating_add(std::time::Duration::from_secs_f64(view.delay).as_nanos() as u64)
}
fn run() -> Result<(), Error> {
    let args = Top::parse();
    hyper_os::require_core_abi()?;
    let mut startup = hyper_rt::process::startup()?;
    let mut session = if args.batch || !std::io::stdout().is_terminal() {
        None
    } else {
        Some(Session::open(&mut startup)?)
    };
    let tasks = TaskInspector::from_handle(startup.take(startup::TASK_INSPECTOR)?);
    let memory = MemoryInspector::from_handle(startup.take(startup::MEMORY_INSPECTOR)?);
    let cpu = CpuInspector::from_handle(startup.take(startup::CPU_INSPECTOR)?);
    let mut sampler = Sampler::default();
    let mut view = View::new(&args);
    // Publish the first complete snapshot before starting any refresh wait.
    let mut frame = sampler.sample(capture(&tasks, &cpu, &memory)?);
    let mut iterations = 1_u32;
    let mut dirty = true;
    loop {
        if dirty {
            render(&frame, &args, &mut view, session.as_ref())?;
            dirty = false;
            if args
                .iterations
                .is_some_and(|limit| iterations >= limit.get())
            {
                return Ok(());
            }
        }
        let mut refresh = false;
        let mut had_input = false;
        if let Some(session) = session.as_mut() {
            // Process one buffered key per turn, so typeahead cannot starve sampling.
            if let Some(key) = session.poll()? {
                had_input = true;
                match view.key(key, session.rows.saturating_sub(7)) {
                    Action::Quit => return Ok(()),
                    Action::Refresh => refresh = true,
                    Action::Redraw => dirty = true,
                    Action::None => {}
                }
            }
            dirty |= std::mem::take(&mut session.resized);
        }
        let deadline = deadline(&frame, &view);
        let now = now()?;
        if refresh || now >= deadline {
            frame = sampler.sample(capture(&tasks, &cpu, &memory)?);
            iterations = iterations.saturating_add(1);
            dirty = true;
        } else if !dirty && !had_input {
            if let Some(session) = session.as_ref() {
                session.wait_until(None, deadline)?;
            } else {
                std::thread::sleep(std::time::Duration::from_nanos(deadline - now));
            }
        }
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("top: {error}");
            ExitCode::FAILURE
        }
    }
}
