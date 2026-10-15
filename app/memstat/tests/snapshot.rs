// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn sample(start: u64) -> Snapshot {
    let mut sample = Snapshot::new(start, start + 1, false);
    sample.add("owner.kernel.resident_pages", Kind::Gauge, 20);
    sample.add("owner.kernel.allocations", Kind::Counter, 100);
    sample
}

#[test]
fn comparison_separates_released_pages_from_allocation_traffic()
-> Result<(), Box<dyn std::error::Error>> {
    let old = sample(10);
    let mut new = sample(20);
    new.add("owner.kernel.resident_pages", Kind::Gauge, 12);
    new.add("owner.kernel.allocations", Kind::Counter, 200);
    new.validate_baseline(&old)?;
    let mut out = Vec::new();
    new.print(Some(&old), false, &mut out)?;
    let text = String::from_utf8(out)?;
    assert!(text.contains("-8"));
    assert!(text.contains("+100"));
    assert!(text.contains("counter"));
    assert!(text.contains("gauge"));
    Ok(())
}

#[test]
fn comparisons_reject_counter_reset_time_reversal_and_configuration_changes() {
    let old = sample(10);
    assert!(sample(1).validate_baseline(&old).is_err());
    let mut new = sample(20);
    new.add("owner.kernel.allocations", Kind::Counter, 99);
    assert!(new.validate_baseline(&old).is_err());
    let mut new = sample(20);
    new.heap_profiling = true;
    assert!(new.validate_baseline(&old).is_err());
    let mut new = sample(20);
    new.metrics.remove("owner.kernel.resident_pages");
    assert!(new.validate_baseline(&old).is_err());
}

#[test]
fn baseline_input_is_bounded_and_versioned() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(&sample(10))?;
    let parsed = Snapshot::load(bytes.as_slice())?;
    assert_eq!(parsed.metrics.len(), 2);
    assert!(Snapshot::load(std::io::repeat(b' ')).is_err());
    let mut future = sample(10);
    future.version = 2;
    assert!(Snapshot::load(serde_json::to_vec(&future)?.as_slice()).is_err());
    Ok(())
}
