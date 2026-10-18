// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn report() -> abi::HyperNativeDeviceDmaFaultInfo {
    abi::HyperNativeDeviceDmaFaultInfo {
        timestamp_ns: 123,
        address: 0x1234,
        stream_id: 16,
        source: 1,
        scope: 1,
        code: 0x10,
        flags: 5,
        reserved: 0,
    }
}

#[test]
fn dma_write_and_controller_reports_keep_their_distinct_meaning() {
    let Some(fault) = decode(report()).unwrap() else {
        panic!("missing report")
    };
    assert_eq!(fault.address, Some(0x1234));
    assert_eq!(fault.read, Some(false));
    assert_eq!(fault.timestamp_ns, 123);
    let mut record = report();
    record.scope = 2;
    record.code = 0;
    record.flags = 0;
    record.address = 0;
    let Some(fault) = decode(record).unwrap() else {
        panic!("missing report")
    };
    assert_eq!(fault.scope, DmaFaultScope::Controller);
    assert_eq!(fault.address, None);
    assert_eq!(fault.read, None);
}

#[test]
fn malformed_or_cross_device_evidence_is_rejected() {
    for (scope, flags, reserved) in [
        (0, 5, 0),
        (3, 5, 0),
        (1, 7, 0),
        (1, 4, 0),
        (2, 5, 0),
        (1, 5, 1),
    ] {
        let mut record = report();
        record.scope = scope;
        record.flags = flags;
        record.reserved = reserved;
        assert_eq!(decode(record), Err(Error::InvalidResponse));
    }
}

#[test]
fn absent_report_requires_an_entirely_empty_record() {
    let record = abi::HyperNativeDeviceDmaFaultInfo {
        timestamp_ns: 0,
        address: 0,
        stream_id: 0,
        source: 0,
        scope: 0,
        code: 0,
        flags: 0,
        reserved: 0,
    };
    assert_eq!(decode(record), Ok(None));
}
