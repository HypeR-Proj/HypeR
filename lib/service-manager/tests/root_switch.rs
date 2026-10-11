// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn policy() -> serde_json::Value {
    serde_json::json!({
        "new-root": "/.newroot",
        "put-old": "/boot",
        "directories": ["/tmp"],
        "links": [
            {"path": "/bin", "target": "/boot/bin"},
            {"path": "/lib", "target": "lib64"}
        ],
        "mounts": [
            {"source": "/system", "target": "/system"},
            {"source": "/data", "target": "/data", "optional": true}
        ]
    })
}

fn valid(value: serde_json::Value) -> bool {
    serde_json::from_value::<Config>(value).is_ok_and(|config| config.validate().is_ok())
}

#[test]
fn accepts_disk_or_ramfs_system_with_optional_data() {
    assert!(valid(policy()));
    let mut value = policy();
    value["mounts"][0]["ramfs"] = true.into();
    assert!(valid(value));
}

#[test]
fn rejects_staging_aliases_and_non_normalized_paths() {
    for path in [
        "/",
        "relative",
        "/./newroot",
        "/old/../newroot",
        "//newroot",
        "/newroot/",
    ] {
        let mut value = policy();
        value["new-root"] = path.into();
        assert!(!valid(value), "accepted {path}");
    }
    for source in ["/", "/.newroot", "/.newroot/system"] {
        let mut value = policy();
        value["mounts"][0]["source"] = source.into();
        assert!(!valid(value), "accepted {source}");
    }
}

#[test]
fn rejects_overlapping_mounts_and_links_before_handoff() {
    for target in ["/boot", "/boot/system", "/data", "/data/system"] {
        let mut value = policy();
        value["mounts"][0]["target"] = target.into();
        assert!(!valid(value), "accepted {target}");
    }
    for path in ["/boot/bin", "/system", "/data/tools", "/tmp", "/lib"] {
        let mut value = policy();
        value["links"][0]["path"] = path.into();
        assert!(!valid(value), "accepted {path}");
    }
}

#[test]
fn rejects_unbounded_or_unrecognized_policy() {
    let mut value = policy();
    value["surprise"] = true.into();
    assert!(!valid(value));
    let mut value = policy();
    value["directories"] = serde_json::json!(
        (0..17)
            .map(|i| format!("/directory{i}"))
            .collect::<Vec<_>>()
    );
    assert!(!valid(value));
    let mut value = policy();
    value["directories"] = serde_json::json!([format!("/{}", "x".repeat(MAX_PATH_BYTES - 1))]);
    assert!(!valid(value));
}

fn nested_policy() -> serde_json::Value {
    let mut value = policy();
    value["mounts"] = serde_json::json!([
        {"source": "/system", "target": "/system"},
        {"source": "/data", "target": "/data", "optional": true},
        {"source": "/data/media", "target": "/data/media", "optional": true}
    ]);
    value
}

#[test]
fn nested_mounts_must_preserve_the_relative_destination() {
    assert!(valid(nested_policy()));
    let mut value = nested_policy();
    value["mounts"][1]["target"] = "/storage".into();
    value["mounts"][2]["target"] = "/storage/media".into();
    assert!(valid(value.clone()));
    value["mounts"][2]["target"] = "/storage/elsewhere".into();
    assert!(!valid(value));
    let mut value = policy();
    value["mounts"][1] = serde_json::json!({"source": "/tmp", "target": "/tmp", "optional": true});
    assert!(valid(value));
}

#[test]
fn move_only_existing_ancestors_and_keep_children_attached() -> Result<(), String> {
    let config: Config =
        serde_json::from_value(nested_policy()).map_err(|error| error.to_string())?;
    let selected = config.selected_mounts(&[SourceState::Mounted; 3])?;
    assert_eq!(
        selected
            .iter()
            .map(|mount| mount.source.as_str())
            .collect::<Vec<_>>(),
        ["/system", "/data"]
    );
    // Selection does not depend on fstab order.
    let mut config = config;
    config.mounts.swap(1, 2);
    config.validate()?;
    let selected = config.selected_mounts(&[SourceState::Mounted; 3])?;
    assert_eq!(
        selected
            .iter()
            .map(|mount| mount.source.as_str())
            .collect::<Vec<_>>(),
        ["/system", "/data"]
    );
    Ok(())
}

#[test]
fn missing_optional_ancestor_cannot_hide_a_mounted_child() -> Result<(), String> {
    let config: Config =
        serde_json::from_value(nested_policy()).map_err(|error| error.to_string())?;
    for state in [SourceState::Directory, SourceState::Missing] {
        let selected =
            config.selected_mounts(&[SourceState::Mounted, state, SourceState::Mounted])?;
        assert_eq!(
            selected
                .iter()
                .map(|mount| mount.source.as_str())
                .collect::<Vec<_>>(),
            ["/system", "/data/media"]
        );
    }
    // An ordinary directory below a mounted parent is not another mount.
    let selected = config.selected_mounts(&[
        SourceState::Mounted,
        SourceState::Mounted,
        SourceState::Directory,
    ])?;
    assert_eq!(
        selected
            .iter()
            .map(|mount| mount.source.as_str())
            .collect::<Vec<_>>(),
        ["/system", "/data"]
    );
    Ok(())
}

#[test]
fn ramfs_transfer_refuses_to_abandon_mounted_descendants() -> Result<(), String> {
    let mut value = nested_policy();
    value["mounts"][1]["ramfs"] = true.into();
    let config: Config = serde_json::from_value(value).map_err(|error| error.to_string())?;
    config.validate()?;
    let result = config.selected_mounts(&[
        SourceState::Mounted,
        SourceState::Directory,
        SourceState::Mounted,
    ]);
    assert!(
        result
            .err()
            .is_some_and(|error| error.contains("transfer cannot contain mounted descendants"))
    );
    assert!(
        config
            .selected_mounts(&[
                SourceState::Mounted,
                SourceState::Directory,
                SourceState::Missing
            ])
            .is_ok()
    );
    Ok(())
}
