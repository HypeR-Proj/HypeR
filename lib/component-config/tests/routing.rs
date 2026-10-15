// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_component_config::Topology;
use serde_json::{Value, json};

fn protocol() -> Value {
    json!({"type":"protocol", "protocol":"hyper.test.Echo"})
}
fn documents() -> Vec<Value> {
    vec![
        json!({"name":"root", "children":[{"name":"client","component":"client"},{"name":"services","component":"services"}],
            "offer":[{"name":"echo","to":"client","from":"#services","source":"echo","kind":protocol()}]}),
        json!({"name":"services", "children":[{"name":"server","component":"server"}],
            "expose":[{"name":"echo","from":"#server","source":"public","kind":protocol()}]}),
        json!({"name":"server", "program":{"image":"/svc/echo"},
            "capabilities":[{"name":"implementation","kind":protocol()}],
            "expose":[{"name":"public","from":"self","source":"implementation","kind":protocol()}]}),
        json!({"name":"client", "program":{"image":"/bin/client"}, "use":[{"name":"echo","kind":protocol()}]}),
    ]
}
fn parse(values: Vec<Value>) -> Result<Topology, String> {
    Topology::parse(
        "root",
        values.into_iter().map(|v| {
            (
                format!("{}.json", v["name"].as_str().unwrap_or("")),
                v.to_string().into_bytes(),
            )
        }),
    )
}

#[test]
fn nested_realms_require_each_boundary_and_resolve_local_names() -> Result<(), String> {
    let graph = parse(documents())?;
    let consumer = graph.index("/client").ok_or("no client")?;
    let route = graph.route(consumer, "echo").ok_or("no route")?;
    assert_eq!(graph.instances[route.provider].path, "/services/server");
    assert_eq!(route.capability, "implementation");
    assert_eq!(route.steps.len(), 4);
    assert!(graph.route(consumer, "implementation").is_none());
    Ok(())
}

#[test]
fn absent_exposure_cannot_be_bypassed_by_knowing_provider_path() {
    let mut d = documents();
    d[1]["expose"] = json!([]);
    assert!(parse(d).is_err());
    let mut d = documents();
    d[0]["offer"][0]["from"] = json!("/services/server");
    assert!(parse(d).is_err());
}

#[test]
fn protocol_mismatch_is_rejected_before_activation() {
    let mut d = documents();
    d[2]["capabilities"][0]["kind"]["protocol"] = json!("hyper.test.Admin");
    assert!(parse(d).is_err());
}

#[test]
fn same_definition_can_have_distinct_instance_identities() -> Result<(), String> {
    let mut d = documents();
    d[1]["children"]
        .as_array_mut()
        .ok_or("children")?
        .push(json!({"name":"other","component":"server"}));
    let graph = parse(d)?;
    assert_ne!(
        graph.index("/services/server"),
        graph.index("/services/other")
    );
    Ok(())
}

#[test]
fn recursive_definitions_and_duplicate_child_names_are_rejected() {
    let mut d = documents();
    d[1]["children"][0]["component"] = json!("root");
    assert!(parse(d).is_err());
    let mut d = documents();
    d[0]["children"][1]["name"] = json!("client");
    assert!(parse(d).is_err());
}

#[test]
fn routing_cycles_are_separate_from_instance_tree_cycles() {
    let d = vec![
        json!({"name":"root","children":[{"name":"a","component":"a"}],
            "offer":[{"name":"loop","to":"a","from":"#a","source":"loop","kind":protocol()}]}),
        json!({"name":"a","use":[{"name":"loop","kind":protocol()}],
            "expose":[{"name":"loop","from":"parent","source":"loop","kind":protocol()}]}),
    ];
    assert!(parse(d).is_err());
}

fn object_documents(transfer: &str) -> Vec<Value> {
    let kind = json!({"type":"object","object_type":1,"rights":7});
    vec![
        json!({"name":"root","children":[{"name":"a","component":"a"}],
            "capabilities":[{"name":"directory","kind":kind,"transfer":transfer}],
            "offer":[{"name":"data","to":"a","from":"self","source":"directory","kind":kind}]}),
        json!({"name":"a","program":{"image":"/bin/a"},"use":[{"name":"data","kind":{"type":"object","object_type":1,"rights":3},"purpose":123}]}),
    ]
}

#[test]
fn object_rights_can_only_be_attenuated_at_each_boundary() -> Result<(), String> {
    let d = object_documents("duplicate");
    parse(d.clone())?;
    let mut d = d;
    d[0]["offer"][0]["kind"]["rights"] = json!(1);
    assert!(parse(d).is_err());
    Ok(())
}

#[test]
fn move_capability_has_one_consumer_and_no_automatic_restart() {
    let mut d = object_documents("move");
    d[1]["program"]["restart"] = json!("on-failure");
    assert!(parse(d).is_err());
    let mut d = object_documents("move");
    d[0]["children"] = json!([{"name":"a","component":"a"},{"name":"b","component":"a"}]);
    let mut offer = d[0]["offer"][0].clone();
    offer["to"] = json!("b");
    d[0]["offer"] = json!([d[0]["offer"][0], offer]);
    assert!(parse(d).is_err());
}

#[test]
fn old_ordering_language_is_not_accepted() {
    let mut d = documents();
    d[3]["after"] = json!(["server"]);
    assert!(parse(d).is_err());
}

#[test]
fn duplicate_native_purposes_and_protocol_purposes_are_rejected() {
    let mut d = object_documents("duplicate");
    let mut usage = d[1]["use"][0].clone();
    usage["name"] = json!("other");
    d[1]["use"] = json!([d[1]["use"][0], usage]);
    assert!(parse(d).is_err());
    let mut d = documents();
    d[3]["use"][0]["purpose"] = json!(12);
    assert!(parse(d).is_err());
}

#[test]
fn filenames_and_size_limits_are_admission_checks() {
    assert!(
        Topology::parse(
            "root",
            [(
                "../root.json".into(),
                json!({"name":"root"}).to_string().into_bytes()
            )]
        )
        .is_err()
    );
    assert!(Topology::parse("root", [("root.json".into(), vec![b' '; 16385])]).is_err());
}

#[test]
fn unused_routes_are_validated_and_framework_ports_cannot_be_overwritten() {
    let mut d = documents();
    d[3]["use"] = json!([]);
    d[2]["expose"][0]["source"] = json!("missing");
    assert!(parse(d).is_err());
    for purpose in [
        4,
        5,
        10,
        15,
        0x80030001,
        0x80030002,
        0x80030003,
        0x80030011,
        0x80030012,
        0x80030013,
        0x800c0001_u32,
        0x800c0002,
        0x800c0020,
    ] {
        let mut d = object_documents("duplicate");
        d[1]["use"][0]["purpose"] = json!(purpose);
        assert!(parse(d).is_err());
    }
}

#[test]
fn persistent_object_exports_and_prepared_grants_have_explicit_owners() {
    let mut d = object_documents("duplicate");
    d[0]["program"] = json!({"image":"/svc/realm"});
    assert!(parse(d).is_err());
    let mut d = object_documents("duplicate");
    d[1]["program"]["prepared"] = json!(true);
    assert!(parse(d).is_err());
}
