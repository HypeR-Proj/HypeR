// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic Rust output for a validated ABI schema.

use super::super::names::{upper_camel, upper_snake};
use super::super::schema;
use super::transfer_class_constant;
use schema::{AbiSchema, FieldKind};
use std::fmt::Write as _;

pub(in super::super) fn render_rust(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "// SPDX-FileCopyrightText: 2026 roolrz\n\
         // SPDX-License-Identifier: Apache-2.0\n\n\
         // Generated from schema/native.rs. Do not edit.\n\n",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_ABI_REVISION: u64 = {};",
        schema.revision
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_INVOCATION_ARGUMENT_REGISTERS: usize = {};",
        schema::INVOCATION_ARGUMENT_REGISTERS
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_INVOCATION_RESULT_REGISTERS: usize = {};",
        schema::INVOCATION_RESULT_REGISTERS
    );
    output.push_str(
        "pub type HyperNativeHandle = u64;\n\
         pub type HyperNativeStatus = i64;\n\n",
    );
    render_rust_bit_constants(
        &mut output,
        "HYPER_NATIVE_FEATURE",
        schema.features.iter().map(|value| (value.name, value.bit)),
    );
    render_rust_i64_constants(
        &mut output,
        "HYPER_NATIVE_STATUS",
        schema
            .statuses
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_rust_u32_constants(
        &mut output,
        "HYPER_NATIVE_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_rust_transfer_classes(&mut output, schema);
    render_rust_bit_constants(
        &mut output,
        "HYPER_NATIVE_RIGHT",
        schema.rights.iter().map(|value| (value.name, value.bit)),
    );
    let rights_mask = schema
        .rights
        .iter()
        .fold(0u64, |mask, right| mask | (1u64 << right.bit));
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_RIGHTS_MASK: u64 = {rights_mask:#x};\n"
    );
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "pub const HYPER_NATIVE_SIGNAL_{}_{}: u64 = 1_u64 << {};",
            upper_snake(signal.object),
            upper_snake(signal.name),
            signal.bit
        );
    }
    if !schema.signals.is_empty() {
        output.push('\n');
    }
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE",
        schema
            .constants
            .iter()
            .map(|constant| (constant.name, constant.value)),
    );
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE_OP",
        schema
            .invocations
            .iter()
            .map(|value| (value.name, u64::from(value.operation))),
    );
    render_rust_invocations(&mut output, schema);
    for record in schema.records {
        let rust_name = upper_camel(record.name);
        let _ = writeln!(
            output,
            "pub const HYPER_NATIVE_{}_MIN_SIZE: usize = {};",
            upper_snake(record.name),
            record.minimum_size
        );
        output.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
        let _ = writeln!(output, "pub struct HyperNative{rust_name} {{");
        let mut cursor = 0u16;
        let mut padding = 0usize;
        for field in record.fields {
            if field.offset > cursor {
                let _ = writeln!(
                    output,
                    "    pub _padding{padding}: [u8; {}],",
                    field.offset - cursor
                );
                padding += 1;
            }
            let _ = writeln!(
                output,
                "    pub {}: {},",
                field.name,
                rust_field_type(field.kind)
            );
            cursor = field.offset + field.kind.size();
        }
        if record.size > cursor {
            let _ = writeln!(
                output,
                "    pub _padding{padding}: [u8; {}],",
                record.size - cursor
            );
        }
        output.push_str("}\n");
        let _ = writeln!(
            output,
            "const _: () = assert!(core::mem::size_of::<HyperNative{rust_name}>() == {});",
            record.size
        );
        let _ = writeln!(
            output,
            "const _: () = assert!(core::mem::align_of::<HyperNative{rust_name}>() == {});",
            record.alignment
        );
        for field in record.fields {
            let assertion = format!(
                "assert!(core::mem::offset_of!(HyperNative{rust_name}, {}) == {});",
                field.name, field.offset
            );
            if "const _: () = ".len() + assertion.len() <= 100 {
                let _ = writeln!(output, "const _: () = {assertion}");
            } else if "    ".len() + assertion.len() <= 100 {
                let _ = writeln!(output, "const _: () =\n    {assertion}");
            } else {
                let _ = writeln!(
                    output,
                    "const _: () = assert!(\n    core::mem::offset_of!(HyperNative{rust_name}, {}) == {}\n);",
                    field.name, field.offset
                );
            }
        }
        output.push('\n');
    }
    if output.ends_with("\n\n") {
        output.pop();
    }
    output
}

fn render_rust_transfer_classes(output: &mut String, schema: &AbiSchema) {
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN: u32 = 0;\n");
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_GENERAL: u32 = 1;\n");
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY: u32 = 2;\n\n");
    output.push_str("pub const fn hyper_native_object_transfer_class(object_kind: u32) -> u32 {\n");
    output.push_str("    match object_kind {\n");
    for object in schema.object_kinds {
        let object_constant = format!("HYPER_NATIVE_OBJECT_{}", upper_snake(object.name));
        let class_constant = transfer_class_constant(object.transfer);
        let arm = format!("        {object_constant} => {class_constant},");
        if arm.len() <= 100 {
            let _ = writeln!(output, "{arm}");
        } else {
            let _ = writeln!(
                output,
                "        {object_constant} => {{\n            {class_constant}\n        }}"
            );
        }
    }
    output.push_str("        _ => HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN,\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
}

fn render_rust_invocations(output: &mut String, schema: &AbiSchema) {
    output.push_str("/// A decoded object method; enum discriminants are not a machine ABI.\n");
    output.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum NativeOperation {\n");
    for invocation in schema.invocations {
        let _ = writeln!(output, "    {},", upper_camel(invocation.name));
    }
    output.push_str("}\n\n#[rustfmt::skip]\nimpl NativeOperation {\n");
    output.push_str(
        "    pub const fn from_raw(object_kind: u32, operation: u64) -> Option<Self> {\n",
    );
    output.push_str("        match object_kind {\n");
    for object in schema
        .object_kinds
        .iter()
        .filter(|object| object.value != 0)
    {
        let _ = writeln!(
            output,
            "            HYPER_NATIVE_OBJECT_{} => {{}},",
            upper_snake(object.name)
        );
    }
    output.push_str("            _ => return None,\n        }\n");
    output.push_str("        match (object_kind, operation) {\n");
    for invocation in schema.invocations {
        let kind = match invocation.receiver.handle.map(|handle| handle.object) {
            Some(schema::ObjectConstraint::Kind(kind)) => {
                format!("HYPER_NATIVE_OBJECT_{}", upper_snake(kind))
            }
            _ => String::from("_"),
        };
        let _ = writeln!(
            output,
            "            ({kind}, HYPER_NATIVE_OP_{}) => Some(Self::{}),",
            upper_snake(invocation.name),
            upper_camel(invocation.name)
        );
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");
    output.push_str("    pub const fn operation(self) -> u64 {\n        match self {\n");
    for invocation in schema.invocations {
        let _ = writeln!(
            output,
            "            Self::{} => HYPER_NATIVE_OP_{},",
            upper_camel(invocation.name),
            upper_snake(invocation.name)
        );
    }
    output.push_str("        }\n    }\n\n");
    output
        .push_str("    pub const fn receiver_kind(self) -> Option<u32> {\n        match self {\n");
    for invocation in schema.invocations {
        let kind = match invocation.receiver.handle.map(|handle| handle.object) {
            Some(schema::ObjectConstraint::Kind(kind)) => {
                format!("Some(HYPER_NATIVE_OBJECT_{})", upper_snake(kind))
            }
            _ => String::from("None"),
        };
        let _ = writeln!(
            output,
            "            Self::{} => {kind},",
            upper_camel(invocation.name)
        );
    }
    output.push_str("        }\n    }\n\n");
    output.push_str("    pub const fn argument_count(self) -> usize {\n        match self {\n");
    for invocation in schema.invocations {
        let _ = writeln!(
            output,
            "            Self::{} => {},",
            upper_camel(invocation.name),
            invocation.arguments.len()
        );
    }
    output.push_str("        }\n    }\n\n");
    output.push_str("    pub const fn receiver_rights(self) -> u64 {\n        match self {\n");
    for invocation in schema.invocations {
        let rights = invocation
            .receiver
            .handle
            .map_or(0, |handle| handle.required_rights);
        let _ = writeln!(
            output,
            "            Self::{} => {rights},",
            upper_camel(invocation.name)
        );
    }
    output.push_str("        }\n    }\n\n");
    output.push_str("    pub const fn failure_result_mask(self, status: HyperNativeStatus) -> u64 {\n        match (self, status) {\n");
    for invocation in schema.invocations {
        for failure in invocation.failure_results {
            let mut mask = 0u64;
            for result_name in failure.results {
                if let Some(index) = invocation
                    .results
                    .iter()
                    .position(|result| result.name == *result_name)
                {
                    mask |= 1u64 << index;
                }
            }
            let _ = writeln!(
                output,
                "            (Self::{}, HYPER_NATIVE_STATUS_{}) => {mask},",
                upper_camel(invocation.name),
                upper_snake(failure.status)
            );
        }
    }
    output.push_str("            _ => 0,\n        }\n    }\n}\n\n");
    output.push_str("pub const fn hyper_native_failure_result_mask(\n    object_kind: u32,\n    operation: u64,\n    status: HyperNativeStatus,\n) -> u64 {\n    match NativeOperation::from_raw(object_kind, operation) {\n        Some(method) => method.failure_result_mask(status),\n        None => 0,\n    }\n}\n\n");
}

fn render_rust_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u64)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: u64 = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_rust_bit_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u8)>,
) {
    for (name, bit) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: u64 = 1_u64 << {bit};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_rust_u32_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u32)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: u32 = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_rust_i64_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, i64)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: HyperNativeStatus = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn rust_field_type(kind: FieldKind) -> String {
    match kind {
        FieldKind::I64 => String::from("i64"),
        FieldKind::U32 => String::from("u32"),
        FieldKind::U64 => String::from("u64"),
        FieldKind::Bytes(size) => format!("[u8; {size}]"),
    }
}
