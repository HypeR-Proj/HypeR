// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Handle transfer, indirect ownership, and object constraints.

use super::records::require_record_field;
use super::{Error, invalid, schema, validate_identifier};
use schema::{
    AbiSchema, CapabilityCommit, FieldKind, HandleOperation, IndirectHandles, MemoryDirection,
    MemoryLength, OperationDisposition,
};
use std::collections::BTreeSet;

pub(super) fn validate_indirect_handles(
    invocation: &schema::Invocation,
    argument: &schema::Argument,
    memory: schema::UserMemory,
    handles: IndirectHandles,
    record: Option<&schema::Record>,
    supported_rights: u64,
) -> Result<(), Error> {
    if !matches!(memory.length, MemoryLength::Elements { .. }) {
        return invalid(format!(
            "invocation {} memory argument {} describes handles without element length",
            invocation.name, argument.name
        ));
    }
    match handles {
        IndirectHandles::BorrowRecords {
            handle_field,
            required_rights,
        } => {
            if memory.direction != MemoryDirection::Read {
                return invalid(format!(
                    "invocation {} borrowed-handle argument {} is not input memory",
                    invocation.name, argument.name
                ));
            }
            if required_rights & !supported_rights != 0 {
                return invalid(format!(
                    "invocation {} argument {} indirectly requires undeclared rights",
                    invocation.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "invocation {} borrowed-handle argument {} has no record",
                    invocation.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, invocation, argument)?;
        }
        IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            operation_field,
            common_rights,
            operations,
            commit: CapabilityCommit::AtomicOnOk,
        } => {
            if memory.direction != MemoryDirection::Read {
                return invalid(format!(
                    "invocation {} consumed-handle argument {} is not input memory",
                    invocation.name, argument.name
                ));
            }
            if common_rights & !supported_rights != 0 {
                return invalid(format!(
                    "invocation {} argument {} indirectly requires undeclared rights",
                    invocation.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "invocation {} consumed-handle argument {} has no record",
                    invocation.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, invocation, argument)?;
            require_record_field(record, rights_field, FieldKind::U64, invocation, argument)?;
            require_record_field(
                record,
                expected_kind_field,
                FieldKind::U32,
                invocation,
                argument,
            )?;
            require_record_field(
                record,
                operation_field,
                FieldKind::U32,
                invocation,
                argument,
            )?;
            validate_distinct_fields(
                invocation,
                argument,
                &[
                    handle_field,
                    rights_field,
                    expected_kind_field,
                    operation_field,
                ],
            )?;
            validate_handle_operations(
                invocation.name,
                argument.name,
                operations,
                supported_rights,
            )?;
            validate_transfer_operations(invocation, argument, common_rights, operations)?;
        }
        IndirectHandles::ProduceTransferred {
            handle_field,
            rights_field,
            expected_kind_field,
            flags_field,
            commit: CapabilityCommit::AtomicOnOk,
        } => {
            if memory.direction != MemoryDirection::ReadWrite {
                return invalid(format!(
                    "invocation {} transferred-handle output {} must be in/out memory",
                    invocation.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "invocation {} transferred-handle output {} has no record",
                    invocation.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, invocation, argument)?;
            require_record_field(record, rights_field, FieldKind::U64, invocation, argument)?;
            require_record_field(
                record,
                expected_kind_field,
                FieldKind::U32,
                invocation,
                argument,
            )?;
            require_record_field(record, flags_field, FieldKind::U32, invocation, argument)?;
            validate_distinct_fields(
                invocation,
                argument,
                &[handle_field, rights_field, expected_kind_field, flags_field],
            )?;
        }
    }
    Ok(())
}

fn validate_distinct_fields(
    invocation: &schema::Invocation,
    argument: &schema::Argument,
    fields: &[&str],
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    if fields.iter().all(|field| names.insert(*field)) {
        Ok(())
    } else {
        invalid(format!(
            "invocation {} memory argument {} repeats transactional record fields",
            invocation.name, argument.name
        ))
    }
}

pub(super) fn validate_transfer_operations(
    invocation: &schema::Invocation,
    argument: &schema::Argument,
    common_rights: u64,
    operations: &[HandleOperation],
) -> Result<(), Error> {
    let move_operation = operations.iter().find(|operation| operation.name == "move");
    let duplicate_operation = operations
        .iter()
        .find(|operation| operation.name == "duplicate");
    if !matches!(
        move_operation,
        Some(HandleOperation {
            value: 0,
            disposition: OperationDisposition::ConsumeOnCommit,
            ..
        })
    ) || common_rights & schema::RIGHT_TRANSFER == 0
    {
        return invalid(format!(
            "invocation {} argument {} has an invalid move operation",
            invocation.name, argument.name
        ));
    }
    if !matches!(
        duplicate_operation,
        Some(HandleOperation {
            value: 1,
            disposition: OperationDisposition::Borrow,
            additional_rights,
            ..
        }) if *additional_rights & schema::RIGHT_DUPLICATE != 0
    ) || common_rights & schema::RIGHT_TRANSFER == 0
    {
        return invalid(format!(
            "invocation {} argument {} has an invalid duplicate operation",
            invocation.name, argument.name
        ));
    }
    Ok(())
}

pub(super) fn validate_handle_operations(
    invocation: &str,
    argument: &str,
    operations: &[HandleOperation],
    supported_rights: u64,
) -> Result<(), Error> {
    if operations.is_empty() {
        return invalid(format!(
            "invocation {invocation} handle {argument} has no operations"
        ));
    }
    let mut names = BTreeSet::new();
    let mut values = BTreeSet::new();
    for operation in operations {
        validate_identifier("handle operation", operation.name)?;
        if !names.insert(operation.name) || !values.insert(operation.value) {
            return invalid(format!(
                "invocation {invocation} handle {argument} repeats a handle operation"
            ));
        }
        if operation.additional_rights & !supported_rights != 0 {
            return invalid(format!(
                "invocation {invocation} handle {argument} operation {} requires undeclared rights",
                operation.name
            ));
        }
    }
    Ok(())
}

pub(super) fn require_object_kind(
    schema: &AbiSchema,
    invocation: &str,
    kind: &str,
) -> Result<(), Error> {
    if schema.object_kinds.iter().any(|object| object.name == kind) {
        Ok(())
    } else {
        invalid(format!(
            "invocation {invocation} names unknown object kind {kind}"
        ))
    }
}
