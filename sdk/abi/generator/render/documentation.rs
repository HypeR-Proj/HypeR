// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Contract prose shared by the Markdown, Rustdoc and Doxygen outputs.

use super::super::schema;
use super::constant_group_kind;
use schema::{
    AbiConstant, AbiSchema, ConstantGroup, ConstantGroupKind, FieldKind, HandleDisposition,
    HandleOperation, IndirectHandles, MemoryLength, ObjectConstraint, OperationDisposition,
    ProducedObject, ProducedRights, TransferClass, ValueKind,
};
use std::fmt::Write as _;

/// Semantic rules are plain prose, not HTML. Preserve literal comparisons and
/// path placeholders when embedding them into Markdown-capable API renderers.
pub(super) fn prose_markdown(prose: &str) -> String {
    prose
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Emit actual API comments, preserving Markdown while preventing schema prose
/// from terminating the C comment. Each source line gets its own comment prefix.
pub(super) fn c_doc(output: &mut String, prose: &str, indent: &str) {
    let _ = writeln!(output, "{indent}/**");
    for line in prose.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            let _ = writeln!(output, "{indent} *");
        } else {
            let _ = writeln!(output, "{indent} * {}", line.replace("*/", "*&#47;"));
        }
    }
    let _ = writeln!(output, "{indent} */");
}

pub(super) fn rust_doc(output: &mut String, prose: &str, indent: &str) {
    for line in prose.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            let _ = writeln!(output, "{indent}///");
        } else {
            let _ = writeln!(output, "{indent}/// {line}");
        }
    }
}

pub(super) fn constant_documentation(group: &ConstantGroup, value: &AbiConstant) -> String {
    let mut prose = format!(
        "`{}` in the `{}` {} domain.\n\n{}\n",
        value.name,
        group.name,
        constant_group_kind(group.kind),
        group.description.trim()
    );
    let rule = match group.kind {
        ConstantGroupKind::Enumeration => "Select one value; this is not a bit mask.",
        ConstantGroupKind::Flags => {
            "Independent flag bit; combine permitted members with bitwise OR."
        }
        ConstantGroupKind::Constants => "Numeric parameter or sentinel, not an enum discriminant.",
    };
    let _ = writeln!(prose, "{rule}");
    if !value.description.trim().is_empty() {
        let _ = writeln!(prose, "\n{}", value.description.trim());
    }
    prose
}

pub(super) fn record_documentation(schema: &AbiSchema, record: &schema::Record) -> String {
    let mut prose = format!(
        "Wire layout for `{}`.\n\nSize: {} bytes; alignment: {} bytes; supported minimum prefix: {} bytes.\n\
         Scalar widths and member offsets are fixed across supported architectures.\n\
         The minimum prefix describes extensibility, not permission to truncate every call's buffer;\n\
         the consuming operation defines its accepted length.\n",
        record.name, record.size, record.alignment, record.minimum_size
    );
    let users = joined_values(
        schema
            .invocations
            .iter()
            .filter(|invocation| {
                invocation.arguments.iter().any(|argument| {
                    argument
                        .memory
                        .is_some_and(|memory| memory.record == Some(record.name))
                })
            })
            .map(|invocation| format!("`{}`", invocation.name)),
    );
    if users != "—" {
        let _ = writeln!(prose, "\nUsed by: {users}.");
    }
    prose
}

pub(super) const TRANSFER_QUERY_DOC: &str = "Return the permitted transport class for an object kind.\n\
     Unknown kinds return FORBIDDEN. This classification does not grant TRANSFER authority.";

pub(super) const FAILURE_MASK_DOC: &str = "Return the auxiliary-result registers defined on a failure status.\n\
     Bit N denotes auxiliary result N, excluding the separate signed status register.\n\
     Zero means no defined failure results (also returned for unknown methods and OK).\n\
     This is not the result-validity mask for a successful call.";

pub(super) const fn transfer_class_name(class: TransferClass) -> &'static str {
    match class {
        TransferClass::Forbidden => "forbidden",
        TransferClass::General => "general",
        TransferClass::RendezvousOnly => "rendezvous_only",
    }
}

pub(super) fn invocation_documentation(invocation: &schema::Invocation) -> String {
    let mut output = String::new();
    let arguments = joined_values(
        invocation
            .arguments
            .iter()
            .map(|value| format!("`{}: {}`", value.name, value_kind_name(value.kind))),
    );
    let results = joined_values(invocation.results.iter().map(|value| {
        let failures = joined_failure_statuses(invocation, value.name);
        format!(
            "`{}: {}{}`",
            value.name,
            value_kind_name(value.kind),
            failures
        )
    }));
    let capability_effects = joined_values(
        invocation
            .inputs()
            .filter_map(|argument| {
                argument
                    .handle
                    .map(|handle| describe_handle_argument(argument.name, handle))
            })
            .chain(invocation.results.iter().filter_map(|result| {
                result
                    .handle
                    .map(|handle| describe_produced_handle(result.name, handle))
            })),
    );
    let user_memory = joined_values(invocation.arguments.iter().filter_map(|argument| {
        argument
            .memory
            .map(|memory| describe_user_memory(argument.name, memory))
    }));
    let execution = format!(
        "`blocking={:?}, cancellation={:?}, restart={:?}, completion={:?}, flags={:?}`",
        invocation.blocking,
        invocation.cancellation,
        invocation.restart,
        invocation.completion,
        invocation.flags
    );
    let _ = writeln!(
        output,
        "Object-local method `{}`.\n\n\
         | Property | Contract |\n\
         | --- | --- |\n\
         | Receiver | {} |\n\
         | Operation | {} |\n\
         | Feature | `{:?}` |\n\
         | Arguments | {} |\n\
         | Results | {} |\n\
         | Capability effects | {} |\n\
         | User memory | {} |\n\
         | Execution | {} |\n\
         | Audit | `{:?}` |\n",
        invocation.name,
        invocation
            .receiver
            .handle
            .map(|handle| describe_handle_argument(invocation.receiver.name, handle))
            .unwrap_or_default(),
        invocation.operation,
        invocation.feature,
        arguments,
        results,
        capability_effects,
        user_memory,
        execution,
        invocation.audit
    );
    output.push_str(
        "Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.\n\n"
    );
    output.push_str("Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.\n");
    output
}

fn joined_values(values: impl Iterator<Item = String>) -> String {
    let values: Vec<_> = values.collect();
    if values.is_empty() {
        String::from("—")
    } else {
        values.join(", ")
    }
}

fn describe_handle_argument(name: &str, handle: schema::HandleArgument) -> String {
    let object = match handle.object {
        ObjectConstraint::Any => String::from("any"),
        ObjectConstraint::Kind(kind) => format!("kind={kind}"),
    };
    match handle.disposition {
        HandleDisposition::Borrow => format!(
            "`{name}: Borrow, {object}, rights=0x{:x}`",
            handle.required_rights
        ),
        HandleDisposition::ConsumeOnCommit => format!(
            "`{name}: ConsumeOnCommit, {object}, rights=0x{:x}`",
            handle.required_rights
        ),
        HandleDisposition::ByOperation {
            argument,
            operations,
        } => format!(
            "`{name}: ByOperation({argument}: {}), {object}, common-rights=0x{:x}`",
            describe_operations(operations, handle.required_rights),
            handle.required_rights
        ),
    }
}

fn describe_produced_handle(name: &str, handle: schema::ProducedHandle) -> String {
    let object = match handle.object {
        ProducedObject::SameAsArgument(argument) => format!("same-as({argument})"),
        ProducedObject::Kind(kind) => format!("kind={kind}"),
    };
    let rights = match handle.rights {
        ProducedRights::FixedAndInherited {
            fixed_rights,
            authority_source,
            inherited_rights,
        } => format!(
            "fixed=0x{fixed_rights:x}, inherited-from({authority_source}), mask=0x{inherited_rights:x}"
        ),
        ProducedRights::RequestedSubsetOf(argument) => format!("subset-from({argument})"),
        ProducedRights::InheritedSubset {
            authority_source,
            allowed_rights,
        } => format!("inherited-from({authority_source}), mask=0x{allowed_rights:x}"),
        ProducedRights::ExactRequested {
            argument,
            allowed_rights,
            authority_source,
        } => {
            let authority = authority_source
                .map_or_else(String::new, |source| format!(", required-from={source}"));
            format!("exact-from({argument}), allowed=0x{allowed_rights:x}{authority}")
        }
        ProducedRights::Fixed(mask) => format!("fixed=0x{mask:x}"),
    };
    format!("`{name}: produce, {object}, {rights}`")
}

fn describe_user_memory(name: &str, memory: schema::UserMemory) -> String {
    let record = memory
        .record
        .map_or_else(String::new, |record| format!(", record={record}"));
    let length = match memory.length {
        MemoryLength::FixedBytes { bytes } => format!("fixed-bytes={bytes}"),
        MemoryLength::Bytes {
            argument,
            maximum_bytes,
        } => format!("len={argument} bytes, max-bytes={maximum_bytes}"),
        MemoryLength::Elements {
            argument,
            maximum_elements,
            element_size,
        } => format!(
            "len={argument} elements, max-elements={maximum_elements}, element-size={element_size}"
        ),
    };
    let handles = match memory.handles {
        None => String::new(),
        Some(IndirectHandles::BorrowRecords {
            handle_field,
            required_rights,
        }) => format!(", borrowed-handles=({handle_field}), required-rights=0x{required_rights:x}"),
        Some(IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            operation_field,
            common_rights,
            operations,
            commit,
        }) => format!(
            ", transactional-handles=({handle_field}, {rights_field}, {expected_kind_field}, {operation_field}), common-rights=0x{common_rights:x}, operations=[{}], commit={commit:?}",
            describe_operations(operations, common_rights)
        ),
        Some(IndirectHandles::ProduceTransferred {
            handle_field,
            rights_field,
            expected_kind_field,
            flags_field,
            commit,
        }) => format!(
            ", typed-receive-slots=({handle_field}, {rights_field}, {expected_kind_field}, {flags_field}), produce-transferred-handles, commit={commit:?}"
        ),
    };
    format!(
        "`{name}: {:?}, {}{}{}; order={}`",
        memory.direction, length, record, handles, memory.validation_order
    )
}

fn describe_operations(operations: &[HandleOperation], common_rights: u64) -> String {
    operations
        .iter()
        .map(|operation| {
            let disposition = match operation.disposition {
                OperationDisposition::Borrow => "Borrow",
                OperationDisposition::ConsumeOnCommit => "ConsumeOnCommit",
            };
            format!(
                "{}={}:{disposition}/rights=0x{:x}",
                operation.name,
                operation.value,
                common_rights | operation.additional_rights
            )
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn joined_failure_statuses(invocation: &schema::Invocation, result_name: &str) -> String {
    let statuses: Vec<_> = invocation
        .failure_results
        .iter()
        .filter(|failure| failure.results.contains(&result_name))
        .map(|failure| failure.status)
        .collect();
    if statuses.is_empty() {
        String::new()
    } else {
        format!("; also-on={}", statuses.join("+"))
    }
}

fn value_kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::U32 => "u32",
        ValueKind::U64 => "u64",
        ValueKind::I64 => "i64",
        ValueKind::Handle => "handle",
        ValueKind::UserAddress => "user_address",
        ValueKind::ByteCount => "byte_count",
        ValueKind::ElementCount => "element_count",
        ValueKind::Rights => "rights",
    }
}

pub(super) fn field_kind_name(kind: FieldKind) -> String {
    match kind {
        FieldKind::I64 => String::from("i64"),
        FieldKind::U32 => String::from("u32"),
        FieldKind::U64 => String::from("u64"),
        FieldKind::Bytes(size) => format!("bytes[{size}]"),
    }
}
