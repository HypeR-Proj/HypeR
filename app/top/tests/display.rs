// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{fixtures::snapshot, sampling::Sampler};
use clap::Parser;

#[test]
fn cpu_modes_preserve_multicore_usage_and_large_counters() -> Result<(), Box<dyn std::error::Error>>
{
    let frame = Sampler::default().sample(snapshot(1000, 2500, 1));
    let args = Top::try_parse_from(["top"])?;
    let mut view = View::new(&args);
    assert!(row(&frame.processes[0], &frame, &view).contains("250.00%"));
    view.normalize = true;
    assert!(row(&frame.processes[0], &frame, &view).contains("62.50%"));
    assert_eq!(percent(u64::MAX, u64::MAX), "100.00%");
    assert_eq!(percent(1, 0), "0.00%");
    assert_eq!(runtime(61_230_000_000), "1:01.23");
    Ok(())
}

#[test]
fn thread_filter_uses_owner_and_names_cannot_inject_terminal_sequences()
-> Result<(), Box<dyn std::error::Error>> {
    let mut sample = snapshot(1000, 100, 3);
    if let Some(thread) = sample.threads.get_mut(&3) {
        thread.name = "bad\x1b[2J\nname".into();
    }
    let frame = Sampler::default().sample(sample);
    let args = Top::try_parse_from(["top", "-H", "-p", "1", "--name", "worker"])?;
    let view = View::new(&args);
    let selected = rows(&frame, &args, &view);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].koid, 3);
    let line = row(&selected[0], &frame, &view);
    assert!(!line.contains('\x1b') && !line.contains('\n'));
    assert!(line.contains("worker [0x0000000000000001]"));
    Ok(())
}

#[test]
fn filtering_and_row_limits_leave_system_summary_intact() -> Result<(), Box<dyn std::error::Error>>
{
    let mut frame = Sampler::default().sample(snapshot(1000, 100, 3));
    frame.processes.push(ProcessSample {
        koid: 2,
        name: "idle".into(),
        state: "running",
        ..Default::default()
    });
    let args = Top::try_parse_from(["top", "--sort", "koid", "--reverse", "--limit", "1"])?;
    let mut view = View::new(&args);
    let selected = rows(&frame, &args, &view);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].koid, 2);
    view.idle_hide = true;
    assert_eq!(rows(&frame, &args, &view)[0].koid, 1);
    assert!(summary(&frame, &view)[3].contains("Processes: 2 total"));
    let args = Top::try_parse_from(["top", "--name", "missing"])?;
    assert!(rows(&frame, &args, &View::new(&args)).is_empty());
    Ok(())
}
