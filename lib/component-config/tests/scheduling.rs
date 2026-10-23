// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_component_config::{FairBackend, Scheduling, SchedulingPolicy, Topology};
use serde_json::json;

#[test]
fn fair_backend_default_does_not_pin_the_kernel_algorithm() -> Result<(), Box<dyn std::error::Error>>
{
    let configuration: Scheduling = serde_json::from_value(json!({
        "policy": {"kind":"fair", "weight":16, "quantum_ns":5_000_000},
        "band":64, "role":"priority"
    }))?;
    assert!(matches!(
        configuration.policy,
        SchedulingPolicy::Fair {
            backend: FairBackend::Default,
            ..
        }
    ));
    assert!(configuration.validate().is_ok());
    Ok(())
}

#[test]
fn malformed_scheduling_is_rejected_before_any_service_launch() {
    for policy in [
        json!({"kind":"fair", "weight":0, "quantum_ns":1}),
        json!({"kind":"budget", "budget_ns":11, "period_ns":10, "burst_ns":1}),
        json!({"kind":"fifo", "priority":256}),
    ] {
        let document = json!({"name":"root", "program": {"image":"/svc/test", "scheduling": {
            "policy":policy, "band":64, "role":"priority"
        }}});
        assert!(
            Topology::parse(
                "root",
                [("root.json".into(), document.to_string().into_bytes())]
            )
            .is_err()
        );
    }
}
