// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic Rust output for a validated ABI schema.

use super::super::names::{upper_camel, upper_snake};
use super::super::schema;
use super::documentation::{
    FAILURE_MASK_DOC, TRANSFER_QUERY_DOC, constant_documentation, invocation_documentation,
    record_documentation, rust_doc,
};
use super::transfer_class_constant;
use schema::{AbiSchema, FieldKind};
use std::fmt::Write as _;

pub(in super::super) fn render_rust(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "// SPDX-FileCopyrightText: 2026 roolrz\n\
         // SPDX-License-Identifier: Apache-2.0\n\n\
         // Generated from schema/native.rs. Do not edit.\n\n",
    );
    rust_doc(
        &mut output,
        "Pre-release schema revision; zero does not promise a stable published ABI.",
        "",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_ABI_REVISION: u64 = {};",
        schema.revision
    );
    rust_doc(
        &mut output,
        "Business argument registers, excluding the receiver and operation selectors.",
        "",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_INVOCATION_ARGUMENT_REGISTERS: usize = {};",
        schema::INVOCATION_ARGUMENT_REGISTERS
    );
    rust_doc(
        &mut output,
        "Auxiliary result registers, excluding the signed status register.",
        "",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_INVOCATION_RESULT_REGISTERS: usize = {};",
        schema::INVOCATION_RESULT_REGISTERS
    );
    output.push_str(
        "/// Process-local, generation-qualified handle; not a KOID or pointer.\n\
         pub type HyperNativeHandle = u64;\n\
         /// Signed invocation status: zero for OK, negative for failure.\n\
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
    rust_doc(
        &mut output,
        "All declared right bits; this mask does not grant authority to a handle.",
        "",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_RIGHTS_MASK: u64 = {rights_mask:#x};\n"
    );
    for signal in schema.signals {
        rust_doc(
            &mut output,
            &format!(
                "Wait signal `{}` for `{}` objects; bit positions are object-local.",
                signal.name, signal.object
            ),
            "",
        );
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
    for group in schema.constant_groups {
        for constant in group.values {
            rust_doc(&mut output, &constant_documentation(group, constant), "");
            let _ = writeln!(
                output,
                "pub const HYPER_NATIVE_{}: u64 = {};",
                upper_snake(constant.name),
                constant.value
            );
        }
        output.push('\n');
    }
    for invocation in schema.invocations {
        rust_doc(&mut output, &invocation_documentation(invocation), "");
        let _ = writeln!(
            output,
            "pub const HYPER_NATIVE_OP_{}: u64 = {};\n",
            upper_snake(invocation.name),
            invocation.operation
        );
    }
    render_rust_invocations(&mut output, schema);
    for record in schema.records {
        let rust_name = upper_camel(record.name);
        rust_doc(
            &mut output,
            &format!(
                "Minimum supported prefix of `{}`, in bytes; individual calls define their buffer lengths.",
                record.name
            ),
            "",
        );
        let _ = writeln!(
            output,
            "pub const HYPER_NATIVE_{}_MIN_SIZE: usize = {};",
            upper_snake(record.name),
            record.minimum_size
        );
        rust_doc(&mut output, &record_documentation(schema, record), "");
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
            rust_doc(
                &mut output,
                &format!(
                    "Wire offset {} bytes; width {} bytes.",
                    field.offset,
                    field.kind.size()
                ),
                "    ",
            );
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
    rust_doc(
        output,
        "Handle transport is forbidden for this object kind.",
        "",
    );
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN: u32 = 0;\n");
    rust_doc(
        output,
        "Buffered capability transfer and rendezvous are permitted with transfer authority.",
        "",
    );
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_GENERAL: u32 = 1;\n");
    rust_doc(
        output,
        "Transfer requires rendezvous or staged startup; buffered transfer is forbidden.",
        "",
    );
    output.push_str("pub const HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY: u32 = 2;\n\n");
    rust_doc(output, TRANSFER_QUERY_DOC, "");
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
        rust_doc(
            output,
            &format!(
                "Decoded method; see [`HYPER_NATIVE_OP_{}`] for its contract.",
                upper_snake(invocation.name)
            ),
            "    ",
        );
        let _ = writeln!(output, "    {},", upper_camel(invocation.name));
    }
    output.push_str("}\n\n#[rustfmt::skip]\nimpl NativeOperation {\n");
    rust_doc(
        output,
        "Decode a declared object kind and object-local operation. Unknown pairs return `None`.\nThis checks vocabulary only, not handle validity or authority.",
        "    ",
    );
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
    rust_doc(
        output,
        "Object-local wire operation number; combine with the receiver kind for dispatch.",
        "    ",
    );
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
    rust_doc(
        output,
        "Required receiver kind, or `None` for a common handle method.",
        "    ",
    );
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
    rust_doc(
        output,
        "Business argument count, excluding the receiver and operation selectors.",
        "    ",
    );
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
    rust_doc(
        output,
        "Unconditional receiver rights. Selector-dependent and indirect-handle checks still apply.",
        "    ",
    );
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
    rust_doc(output, FAILURE_MASK_DOC, "    ");
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
    rust_doc(output, FAILURE_MASK_DOC, "");
    output.push_str("pub const fn hyper_native_failure_result_mask(\n    object_kind: u32,\n    operation: u64,\n    status: HyperNativeStatus,\n) -> u64 {\n    match NativeOperation::from_raw(object_kind, operation) {\n        Some(method) => method.failure_result_mask(status),\n        None => 0,\n    }\n}\n\n");
}

fn render_rust_bit_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u8)>,
) {
    for (name, bit) in values {
        let category = if prefix == "HYPER_NATIVE_RIGHT" {
            "Capability right; the receiving object's method determines the required rights"
        } else {
            "Advertised ABI feature bit"
        };
        rust_doc(output, &format!("{category}: `{name}`."), "");
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
        rust_doc(
            output,
            &format!("Object-kind tag `{name}` used with an object-local operation number."),
            "",
        );
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
        rust_doc(
            output,
            &format!(
                "Invocation status `{name}`. Auxiliary results on failure require an explicit method contract."
            ),
            "",
        );
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
