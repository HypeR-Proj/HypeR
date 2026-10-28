// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Invocation argument, result, and completion contracts.

use super::handles::{
    require_object_kind, validate_handle_operations, validate_indirect_handles,
    validate_transfer_operations,
};
use super::{Error, invalid, schema, validate_identifier};
use schema::{
    AbiSchema, CompletionClass, FeatureGate, HandleDisposition, MemoryLength, ObjectConstraint,
    ProducedObject, ProducedRights, ValueKind,
};
use std::collections::BTreeSet;

pub(super) fn validate_invocations(schema: &AbiSchema, supported_rights: u64) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for invocation in schema.invocations {
        validate_identifier("invocation", invocation.name)?;
        if !names.insert(invocation.name) {
            return invalid(format!(
                "invocation {} is declared more than once",
                invocation.name
            ));
        }
        if invocation.receiver.kind != ValueKind::Handle || invocation.receiver.memory.is_some() {
            return invalid(format!(
                "invocation {} has an invalid receiver",
                invocation.name
            ));
        }
        let Some(receiver) = invocation.receiver.handle else {
            return invalid(format!(
                "invocation {} is missing receiver metadata",
                invocation.name
            ));
        };
        let namespace = match receiver.object {
            ObjectConstraint::Any => {
                if invocation.operation >= 6 {
                    return invalid("common handle operation is outside 0..5");
                }
                "*"
            }
            ObjectConstraint::Kind(kind) => {
                if invocation.operation < 256 {
                    return invalid("object-local operation overlaps the common namespace");
                }
                kind
            }
        };
        if !operations.insert((namespace, invocation.operation)) {
            return invalid(format!(
                "invocation operation {} repeats within {namespace}",
                invocation.operation
            ));
        }
        match invocation.feature {
            FeatureGate::Core => {}
            FeatureGate::Named(name)
                if schema.features.iter().any(|feature| feature.name == name) => {}
            FeatureGate::Named(name) => {
                return invalid(format!(
                    "invocation {} names unknown feature {name}",
                    invocation.name
                ));
            }
        }
        if invocation.arguments.len() >= schema::INVOCATION_ARGUMENT_REGISTERS {
            return invalid(format!(
                "invocation {} must retain a reserved argument register; use an extensible request for additional fields",
                invocation.name
            ));
        }
        if invocation.results.len() > schema::INVOCATION_RESULT_REGISTERS {
            return invalid(format!(
                "invocation {} exceeds the machine result registers",
                invocation.name
            ));
        }
        if invocation.completion == CompletionClass::NoReturn && !invocation.results.is_empty() {
            return invalid(format!(
                "no-return invocation {} declares results",
                invocation.name
            ));
        }
        validate_arguments(schema, invocation, supported_rights)?;
        validate_results(schema, invocation, supported_rights)?;
    }
    Ok(())
}

fn validate_arguments(
    schema: &AbiSchema,
    invocation: &schema::Invocation,
    supported_rights: u64,
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    let mut memory_orders = BTreeSet::new();
    let mut memory_count = 0usize;
    for argument in invocation.inputs() {
        validate_identifier("argument", argument.name)?;
        if !names.insert(argument.name) {
            return invalid(format!(
                "invocation {} repeats argument {}",
                invocation.name, argument.name
            ));
        }
        if argument.handle.is_some() != (argument.kind == ValueKind::Handle) {
            return invalid(format!(
                "invocation {} argument {} has inconsistent handle metadata",
                invocation.name, argument.name
            ));
        }
        if argument.memory.is_some() != (argument.kind == ValueKind::UserAddress) {
            return invalid(format!(
                "invocation {} argument {} has inconsistent memory metadata",
                invocation.name, argument.name
            ));
        }
        if let Some(handle) = argument.handle {
            if handle.required_rights & !supported_rights != 0 {
                return invalid(format!(
                    "invocation {} argument {} requires undeclared rights",
                    invocation.name, argument.name
                ));
            }
            if let ObjectConstraint::Kind(kind) = handle.object {
                require_object_kind(schema, invocation.name, kind)?;
            }
            match handle.disposition {
                HandleDisposition::Borrow | HandleDisposition::ConsumeOnCommit => {}
                HandleDisposition::ByOperation {
                    argument: operation_argument,
                    operations,
                } => {
                    if !invocation.inputs().any(|candidate| {
                        candidate.name == operation_argument && candidate.kind == ValueKind::U32
                    }) {
                        return invalid(format!(
                            "invocation {} handle {} names invalid operation argument {operation_argument}",
                            invocation.name, argument.name
                        ));
                    }
                    validate_handle_operations(
                        invocation.name,
                        argument.name,
                        operations,
                        supported_rights,
                    )?;
                    validate_transfer_operations(
                        invocation,
                        argument,
                        handle.required_rights,
                        operations,
                    )?;
                }
            }
        }
        if let Some(memory) = argument.memory {
            memory_count += 1;
            if !memory_orders.insert(memory.validation_order) {
                return invalid(format!(
                    "invocation {} repeats memory validation order {}",
                    invocation.name, memory.validation_order
                ));
            }
            let (length_argument, expected_length_kind, maximum_bytes, element_size) = match memory
                .length
            {
                MemoryLength::FixedBytes { bytes } => {
                    if bytes == 0 {
                        return invalid("fixed memory size cannot be zero");
                    }
                    ("", ValueKind::ByteCount, bytes, None)
                }
                MemoryLength::Bytes {
                    argument: length_name,
                    maximum_bytes,
                } => {
                    if maximum_bytes == 0 {
                        return invalid(format!(
                            "invocation {} argument {} has an unbounded zero byte maximum",
                            invocation.name, argument.name
                        ));
                    }
                    (length_name, ValueKind::ByteCount, maximum_bytes, None)
                }
                MemoryLength::StridedElements {
                    argument: length_name,
                    maximum_elements,
                    stride_argument,
                    maximum_stride,
                } => {
                    if maximum_elements == 0
                        || maximum_stride == 0
                        || memory.record.is_none()
                        || !invocation
                            .inputs()
                            .any(|a| a.name == stride_argument && a.kind == ValueKind::ByteCount)
                    {
                        return invalid(format!(
                            "invocation {} has invalid record stride",
                            invocation.name
                        ));
                    }
                    let Some(maximum_bytes) =
                        maximum_elements.checked_mul(u32::from(maximum_stride))
                    else {
                        return invalid("strided array extent overflows");
                    };
                    (length_name, ValueKind::ElementCount, maximum_bytes, None)
                }
                MemoryLength::Elements {
                    argument: length_name,
                    maximum_elements,
                    element_size,
                } => {
                    if maximum_elements == 0 || element_size == 0 {
                        return invalid(format!(
                            "invocation {} argument {} has an invalid element bound",
                            invocation.name, argument.name
                        ));
                    }
                    let Some(maximum_bytes) = maximum_elements.checked_mul(u32::from(element_size))
                    else {
                        return invalid(format!(
                            "invocation {} argument {} element range overflows",
                            invocation.name, argument.name
                        ));
                    };
                    (
                        length_name,
                        ValueKind::ElementCount,
                        maximum_bytes,
                        Some(element_size),
                    )
                }
            };
            let length = invocation
                .inputs()
                .find(|candidate| candidate.name == length_argument);
            if !matches!(memory.length, MemoryLength::FixedBytes { .. })
                && !matches!(length, Some(candidate) if candidate.kind == expected_length_kind)
            {
                return invalid(format!(
                    "invocation {} memory argument {} has no matching length argument",
                    invocation.name, argument.name,
                ));
            }
            let mut selected_record = None;
            if let Some(record_name) = memory.record {
                let Some(record) = schema
                    .records
                    .iter()
                    .find(|record| record.name == record_name)
                else {
                    return invalid(format!(
                        "invocation {} memory argument {} names unknown record {record_name}",
                        invocation.name, argument.name
                    ));
                };
                if matches!(memory.length, MemoryLength::FixedBytes { .. }) {
                    return invalid(format!(
                        "invocation {} record {} needs an explicit size argument",
                        invocation.name, record_name
                    ));
                }
                if let MemoryLength::StridedElements { maximum_stride, .. } = memory.length
                    && (maximum_stride < record.size || maximum_stride % 8 != 0)
                {
                    return invalid(format!(
                        "invocation {} has an invalid record stride bound",
                        invocation.name
                    ));
                }
                if u32::from(record.size) > maximum_bytes {
                    return invalid(format!(
                        "invocation {} memory argument {} cannot contain record {record_name}",
                        invocation.name, argument.name
                    ));
                }
                if element_size.is_some_and(|size| size != record.size) {
                    return invalid(format!(
                        "invocation {} memory argument {} element size does not match record {record_name}",
                        invocation.name, argument.name
                    ));
                }
                selected_record = Some(record);
            }
            if u32::from(element_size.unwrap_or(1)) > maximum_bytes {
                return invalid(format!(
                    "invocation {} memory argument {} cannot contain one element",
                    invocation.name, argument.name
                ));
            }
            if let Some(handles) = memory.handles {
                validate_indirect_handles(
                    invocation,
                    argument,
                    memory,
                    handles,
                    selected_record,
                    supported_rights,
                )?;
            }
        }
    }
    if !(0..memory_count).all(|order| memory_orders.contains(&(order as u8))) {
        return invalid(format!(
            "invocation {} memory validation order is not contiguous",
            invocation.name
        ));
    }
    Ok(())
}

fn validate_results(
    schema: &AbiSchema,
    invocation: &schema::Invocation,
    supported_rights: u64,
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for result in invocation.results {
        validate_identifier("result", result.name)?;
        if !names.insert(result.name) {
            return invalid(format!(
                "invocation {} repeats result {}",
                invocation.name, result.name
            ));
        }
        if result.handle.is_some() != (result.kind == ValueKind::Handle) {
            return invalid(format!(
                "invocation {} result {} has inconsistent handle metadata",
                invocation.name, result.name
            ));
        }
        let Some(handle) = result.handle else {
            continue;
        };
        match handle.object {
            ProducedObject::SameAsArgument(argument) => {
                if !invocation.inputs().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Handle
                }) {
                    return invalid(format!(
                        "invocation {} result {} names invalid source handle {argument}",
                        invocation.name, result.name
                    ));
                }
            }
            ProducedObject::Kind(kind) => require_object_kind(schema, invocation.name, kind)?,
        }
        match handle.rights {
            ProducedRights::FixedAndInherited {
                fixed_rights,
                authority_source,
                inherited_rights,
            } => {
                if !invocation.inputs().any(|candidate| {
                    candidate.name == authority_source && candidate.kind == ValueKind::Handle
                }) {
                    return invalid(format!(
                        "invocation {} result {} names invalid inherited-rights source {authority_source}",
                        invocation.name, result.name
                    ));
                }
                if inherited_rights == 0
                    || (fixed_rights | inherited_rights) & !supported_rights != 0
                    || fixed_rights & inherited_rights != 0
                {
                    return invalid(format!(
                        "invocation {} result {} has invalid fixed/inherited rights",
                        invocation.name, result.name
                    ));
                }
            }
            ProducedRights::InheritedSubset {
                authority_source,
                allowed_rights,
            } => {
                if !invocation.inputs().any(|candidate| {
                    candidate.name == authority_source && candidate.kind == ValueKind::Handle
                }) {
                    return invalid(format!(
                        "invocation {} result {} names invalid inherited-rights source {authority_source}",
                        invocation.name, result.name
                    ));
                }
                if allowed_rights == 0 || allowed_rights & !supported_rights != 0 {
                    return invalid(format!(
                        "invocation {} result {} inherits undeclared or empty object rights",
                        invocation.name, result.name
                    ));
                }
            }
            ProducedRights::RequestedSubsetOf(argument) => {
                if !invocation.inputs().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Rights
                }) {
                    return invalid(format!(
                        "invocation {} result {} names invalid rights argument {argument}",
                        invocation.name, result.name
                    ));
                }
            }
            ProducedRights::ExactRequested {
                argument,
                allowed_rights,
                authority_source,
            } => {
                if !invocation.inputs().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Rights
                }) {
                    return invalid(format!(
                        "invocation {} result {} names invalid exact-rights argument {argument}",
                        invocation.name, result.name
                    ));
                }
                if allowed_rights == 0 || allowed_rights & !supported_rights != 0 {
                    return invalid(format!(
                        "invocation {} result {} allows undeclared or empty object rights",
                        invocation.name, result.name
                    ));
                }
                if let Some(source) = authority_source
                    && !invocation.inputs().any(|candidate| {
                        candidate.name == source && candidate.kind == ValueKind::Handle
                    })
                {
                    return invalid(format!(
                        "invocation {} result {} names invalid authority source handle {source}",
                        invocation.name, result.name
                    ));
                }
            }
            ProducedRights::Fixed(rights) if rights & !supported_rights != 0 => {
                return invalid(format!(
                    "invocation {} result {} produces undeclared rights",
                    invocation.name, result.name
                ));
            }
            ProducedRights::Fixed(_) => {}
        }
    }
    let mut failure_statuses = BTreeSet::new();
    for failure in invocation.failure_results {
        validate_identifier("failure-result status", failure.status)?;
        if !schema
            .statuses
            .iter()
            .any(|status| status.name == failure.status && status.value != 0)
        {
            return invalid(format!(
                "invocation {} names unknown failure-result status {}",
                invocation.name, failure.status
            ));
        }
        if !failure_statuses.insert(failure.status) {
            return invalid(format!(
                "invocation {} repeats failure-result status {}",
                invocation.name, failure.status
            ));
        }
        if failure.results.is_empty() {
            return invalid(format!(
                "invocation {} exposes no results for failure status {}",
                invocation.name, failure.status
            ));
        }
        let mut result_names = BTreeSet::new();
        for name in failure.results {
            if !result_names.insert(*name)
                || !invocation.results.iter().any(|result| result.name == *name)
            {
                return invalid(format!(
                    "invocation {} names invalid failure result {}",
                    invocation.name, name
                ));
            }
        }
    }
    Ok(())
}
