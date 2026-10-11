// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn basic() -> Vec<Slot> {
    [
        BOOT_ENVIRONMENT,
        SYSTEM_ENVIRONMENT,
        ROOT_DIRECTORY,
        LIBRARY_DIRECTORY,
        FACTORY,
        GROUP,
        DOMAIN,
        TASK_INSPECTOR,
        OBJECT_INSPECTOR,
        MEMORY_INSPECTOR,
        CPU_INSPECTOR,
        SERVICE_OUTPUT,
        STAGE_GROUP,
        CONSOLE,
    ]
    .into_iter()
    .map(|key| Slot {
        key,
        rights: if matches!(key, BOOT_ENVIRONMENT | SYSTEM_ENVIRONMENT) {
            hyper_service::process::ENVIRONMENT_DIRECTORY_CONTRACT
                .required_rights()
                .bits()
        } else {
            Rights::TRANSFER.bits()
        },
    })
    .collect()
}

#[test]
fn receive_rejects_missing_duplicate_or_unknown_roles_before_accepting_any_owner() {
    let slots = basic();
    assert!(validate_inventory(&slots, 3).is_ok());
    assert!(validate_inventory(&slots[1..], 3).is_err());
    let mut slots = basic();
    slots.push(Slot {
        key: ROOT_DIRECTORY,
        rights: Rights::TRANSFER.bits(),
    });
    assert!(validate_inventory(&slots, 3).is_err());
    if let Some(slot) = slots.last_mut() {
        slot.key = 36;
    }
    assert!(validate_inventory(&slots, 3).is_err());
}

#[test]
fn supervisor_keys_are_bounded_by_the_exact_core_manifest() {
    let mut slots = basic();
    slots.push(Slot {
        key: PROCESS_BASE + 2,
        rights: Rights::TRANSFER.bits(),
    });
    assert!(validate_inventory(&slots, 3).is_ok());
    assert!(validate_inventory(&slots, 2).is_err());
}

fn with_roles(keys: &[u16]) -> Vec<Slot> {
    let mut slots = basic();
    slots.extend(keys.iter().map(|key| Slot {
        key: *key,
        rights: Rights::TRANSFER.bits(),
    }));
    slots
}

#[test]
fn io_vm_authority_and_shell_connection_do_not_require_a_business_fleet() {
    for roles in [
        vec![VM_AUTHORITY],
        vec![VM_CLIENT, VM_MANAGER],
        vec![VM_AUTHORITY, VM_CLIENT, VM_MANAGER],
        vec![VM_CLIENT],
    ] {
        assert!(validate_inventory(&with_roles(&roles), 3).is_ok());
    }
}

#[test]
fn fleet_and_connection_reject_partial_ownership() {
    for roles in [
        vec![VM_GROUP],
        vec![VM_DOMAIN],
        vec![PROVISIONER],
        vec![VM_PROVISIONING],
        vec![VM_MANAGER],
    ] {
        assert!(validate_inventory(&with_roles(&roles), 3).is_err());
    }
    assert!(validate_inventory(&with_roles(&[VM_GROUP, VM_DOMAIN, PROVISIONER]), 3).is_ok());
}

#[test]
fn root_switch_authority_must_be_offered_before_adopting_supervision() -> Result<(), String> {
    let mut slots = basic();
    assert!(validate_namespace_source(&slots).is_err());
    let root = slots
        .iter_mut()
        .find(|slot| slot.key == ROOT_DIRECTORY)
        .ok_or("root role")?;
    root.rights = Rights::MANAGE_NAMESPACE.union(Rights::READ).bits();
    assert!(validate_namespace_source(&slots).is_err());
    let root = slots
        .iter_mut()
        .find(|slot| slot.key == ROOT_DIRECTORY)
        .ok_or("root role")?;
    root.rights = Rights::MANAGE_NAMESPACE
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::EXECUTE)
        .union(Rights::PROTECT)
        .bits();
    assert!(validate_namespace_source(&slots).is_ok());
    Ok(())
}

#[test]
fn vm_authority_is_checked_before_handoff_even_without_a_prepared_fleet() -> Result<(), String> {
    let manifest = crate::manifest::parse(include_str!(
        "../../../bootstrap/stage1/init/config/services-with-vms.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    let plan = crate::manifest::validate(&manifest, &crate::bootstrap_policy::BootstrapPolicy)
        .map_err(|error| format!("{error:?}"))?;
    assert!(validate_vm_sources(&with_roles(&[IO_BROKER_CLIENT]), &plan, 0).is_err());
    assert!(validate_vm_sources(&with_roles(&[VM_AUTHORITY, IO_BROKER_CLIENT]), &plan, 0).is_ok());
    assert!(
        validate_vm_sources(
            &with_roles(&[VM_AUTHORITY, IO_BROKER_CLIENT, VM_CLIENT]),
            &plan,
            0
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn environment_rights_are_checked_before_accepting_supervision() {
    for key in [BOOT_ENVIRONMENT, SYSTEM_ENVIRONMENT] {
        let mut slots = basic();
        for slot in &mut slots {
            if slot.key == key {
                slot.rights = Rights::READ.union(Rights::TRANSFER).bits();
            }
        }
        assert!(validate_inventory(&slots, 0).is_err());
    }
}

#[test]
fn waiting_core_session_keeps_its_decision_writer_across_handoff() -> Result<(), String> {
    let manifest = crate::manifest::parse(include_str!(
        "../../../bootstrap/stage1/init/config/services-console-only.json"
    ))
    .map_err(|error| format!("{error:?}"))?;
    let plan = crate::manifest::validate(&manifest, &crate::bootstrap_policy::BootstrapPolicy)
        .map_err(|error| format!("{error:?}"))?;
    let mut slots = basic();
    assert!(validate_session_source(&slots, &plan, manifest.service_count()).is_err());
    slots.push(Slot {
        key: SESSION_DECISION,
        rights: Rights::READ.bits(),
    });
    assert!(validate_session_source(&slots, &plan, manifest.service_count()).is_err());
    let writer = slots.last_mut().ok_or("writer")?;
    writer.rights = Rights::WRITE.bits();
    assert!(validate_session_source(&slots, &plan, manifest.service_count()).is_ok());
    Ok(())
}
