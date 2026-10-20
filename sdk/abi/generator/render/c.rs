// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic C output for a validated ABI schema.

use super::super::names::upper_snake;
use super::super::schema;
use super::documentation::{
    FAILURE_MASK_DOC, TRANSFER_QUERY_DOC, c_doc, constant_documentation, invocation_documentation,
    prose_markdown, record_documentation,
};
use super::{constant_group_kind, transfer_class_constant};
use schema::{AbiSchema, FieldKind};
use std::fmt::Write as _;

pub(in super::super) fn render_c(schema: &AbiSchema) -> String {
    let mut output = String::from("/* SPDX-FileCopyrightText: 2026 roolrz\n");
    output.push_str(" * SPDX-License-Identifier: Apache-2.0\n");
    output.push_str(" *\n");
    output.push_str(" * Generated from schema/native.rs. Do not edit.\n");
    output.push_str(" */\n\n");
    c_doc(
        &mut output,
        "@file\n@brief Fixed-width HypeR Native capability invocation ABI.\n\
         The receiver handle selects authority; operation numbers are object-local.\n\
         Constants remain integers on the wire, including values grouped as enum domains.\n\
         See @ref hyper_native_contract for ownership and lifecycle rules.\n\
         @defgroup hyper_native HypeR Native ABI\n@{",
        "",
    );
    let mut contract = String::from("@page hyper_native_contract Native ABI semantic contract\n");
    for rule in schema.semantic_rules {
        let _ = writeln!(contract, "\n- {}", prose_markdown(rule));
    }
    c_doc(&mut output, &contract, "");
    output.push_str("#ifndef HYPER_NATIVE_H\n");
    output.push_str("#define HYPER_NATIVE_H\n\n");
    output.push_str("#include <stddef.h>\n#include <stdint.h>\n\n");
    output.push_str("#if defined(__cplusplus)\n");
    output.push_str("#define HYPER_ABI_STATIC_ASSERT static_assert\n");
    output.push_str("#define HYPER_ABI_ALIGNOF alignof\n");
    output.push_str("#else\n");
    output.push_str("#define HYPER_ABI_STATIC_ASSERT _Static_assert\n");
    output.push_str("#define HYPER_ABI_ALIGNOF _Alignof\n");
    output.push_str("#endif\n\n");
    c_doc(
        &mut output,
        "Pre-release schema revision; zero does not promise a stable published ABI.",
        "",
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_ABI_REVISION UINT64_C({})",
        schema.revision
    );
    c_doc(
        &mut output,
        "Business argument registers, excluding the receiver and operation selectors.",
        "",
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_INVOCATION_ARGUMENT_REGISTERS UINT32_C({})",
        schema::INVOCATION_ARGUMENT_REGISTERS
    );
    c_doc(
        &mut output,
        "Auxiliary result registers, excluding the signed status register.",
        "",
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_INVOCATION_RESULT_REGISTERS UINT32_C({})",
        schema::INVOCATION_RESULT_REGISTERS
    );
    output.push_str(
        "\n/** Process-local, generation-qualified handle; not a KOID or pointer. */\n\
         typedef uint64_t hyper_native_handle_t;\n\
         /** Signed invocation status: zero for OK, negative for failure. */\n\
         typedef int64_t hyper_native_status_t;\n\n",
    );
    render_c_bit_constants(
        &mut output,
        "HYPER_NATIVE_FEATURE",
        schema.features.iter().map(|value| (value.name, value.bit)),
    );
    render_c_i64_constants(
        &mut output,
        "HYPER_NATIVE_STATUS",
        schema
            .statuses
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_c_u32_constants(
        &mut output,
        "HYPER_NATIVE_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_c_transfer_classes(&mut output, schema);
    render_c_bit_constants(
        &mut output,
        "HYPER_NATIVE_RIGHT",
        schema.rights.iter().map(|value| (value.name, value.bit)),
    );
    let rights_mask = schema
        .rights
        .iter()
        .fold(0u64, |mask, right| mask | (1u64 << right.bit));
    c_doc(
        &mut output,
        "All declared right bits; this mask does not grant authority to a handle.",
        "",
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_RIGHTS_MASK UINT64_C({rights_mask:#x})\n"
    );
    for signal in schema.signals {
        c_doc(
            &mut output,
            &format!(
                "Wait signal `{}` for `{}` objects; bit positions are object-local.",
                signal.name, signal.object
            ),
            "",
        );
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_SIGNAL_{}_{} (UINT64_C(1) << {})",
            upper_snake(signal.object),
            upper_snake(signal.name),
            signal.bit
        );
    }
    if !schema.signals.is_empty() {
        output.push('\n');
    }
    for group in schema.constant_groups {
        c_doc(
            &mut output,
            &format!(
                "@defgroup hyper_native_{} {} ({})\n@ingroup hyper_native\n{}\n@{{",
                group.name,
                group.name,
                constant_group_kind(group.kind),
                group.description.trim()
            ),
            "",
        );
        for constant in group.values {
            c_doc(&mut output, &constant_documentation(group, constant), "");
            let _ = writeln!(
                output,
                "#define HYPER_NATIVE_{} UINT64_C({})",
                upper_snake(constant.name),
                constant.value
            );
        }
        output.push_str("/** @} */\n\n");
    }
    for invocation in schema.invocations {
        c_doc(&mut output, &invocation_documentation(invocation), "");
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_OP_{} UINT64_C({})\n",
            upper_snake(invocation.name),
            invocation.operation
        );
    }
    render_c_failure_result_mask(&mut output, schema);
    for record in schema.records {
        c_doc(
            &mut output,
            &format!(
                "Minimum supported prefix of `{}`, in bytes; individual calls define their buffer lengths.",
                record.name
            ),
            "",
        );
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_{}_MIN_SIZE UINT64_C({})",
            upper_snake(record.name),
            record.minimum_size
        );
        c_doc(&mut output, &record_documentation(schema, record), "");
        let _ = writeln!(output, "typedef struct hyper_native_{}_t {{", record.name);
        let mut cursor = 0u16;
        let mut padding = 0usize;
        for field in record.fields {
            if field.offset > cursor {
                let _ = writeln!(
                    output,
                    "    uint8_t _padding{padding}[{}];",
                    field.offset - cursor
                );
                padding += 1;
            }
            c_doc(
                &mut output,
                &format!(
                    "Wire offset {} bytes; width {} bytes.",
                    field.offset,
                    field.kind.size()
                ),
                "    ",
            );
            match field.kind {
                FieldKind::Bytes(size) => {
                    let _ = writeln!(output, "    uint8_t {}[{size}];", field.name);
                }
                kind => {
                    let _ = writeln!(output, "    {} {};", c_scalar_field_type(kind), field.name);
                }
            }
            cursor = field.offset + field.kind.size();
        }
        if record.size > cursor {
            let _ = writeln!(
                output,
                "    uint8_t _padding{padding}[{}];",
                record.size - cursor
            );
        }
        let _ = writeln!(output, "}} hyper_native_{}_t;", record.name);
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_{}_t) == {}, \"{} size\");",
            record.name, record.size, record.name
        );
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_{}_t) == {}, \"{} alignment\");",
            record.name, record.alignment, record.name
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_{}_t, {}) == {}, \"{}.{} offset\");",
                record.name, field.name, field.offset, record.name, field.name
            );
        }
        output.push('\n');
    }
    output.push_str("#undef HYPER_ABI_ALIGNOF\n#undef HYPER_ABI_STATIC_ASSERT\n\n");
    output.push_str("/** @} */\n\n");
    output.push_str("#endif /* HYPER_NATIVE_H */\n");
    output
}

fn render_c_transfer_classes(output: &mut String, schema: &AbiSchema) {
    c_doc(
        output,
        "Handle transport is forbidden for this object kind.",
        "",
    );
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN UINT32_C(0)\n");
    c_doc(
        output,
        "Buffered capability transfer and rendezvous are permitted with transfer authority.",
        "",
    );
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_GENERAL UINT32_C(1)\n");
    c_doc(
        output,
        "Transfer requires rendezvous or staged startup; buffered transfer is forbidden.",
        "",
    );
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY UINT32_C(2)\n\n");
    c_doc(output, TRANSFER_QUERY_DOC, "");
    output.push_str(
        "static inline uint32_t hyper_native_object_transfer_class(uint32_t object_kind) {\n",
    );
    output.push_str("    switch (object_kind) {\n");
    for object in schema.object_kinds {
        let _ = writeln!(
            output,
            "        case HYPER_NATIVE_OBJECT_{}: return {};",
            upper_snake(object.name),
            transfer_class_constant(object.transfer)
        );
    }
    output.push_str("        default: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
}

fn render_c_failure_result_mask(output: &mut String, schema: &AbiSchema) {
    c_doc(output, FAILURE_MASK_DOC, "");
    output.push_str(
        "static inline uint64_t hyper_native_failure_result_mask(\n    uint32_t object_kind, uint64_t operation, hyper_native_status_t status)\n{\n",
    );
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
            if let Some(schema::ObjectConstraint::Kind(kind)) =
                invocation.receiver.handle.map(|handle| handle.object)
            {
                let _ = writeln!(
                    output,
                    "    if (object_kind == HYPER_NATIVE_OBJECT_{} &&\n        operation == HYPER_NATIVE_OP_{} &&\n        status == HYPER_NATIVE_STATUS_{}) {{\n        return UINT64_C({mask});\n    }}",
                    upper_snake(kind),
                    upper_snake(invocation.name),
                    upper_snake(failure.status)
                );
            } else {
                // Common methods accept every declared object kind except NONE,
                // matching NativeOperation::from_raw in the Rust output.
                let _ = writeln!(
                    output,
                    "    if (operation == HYPER_NATIVE_OP_{} &&\n        status == HYPER_NATIVE_STATUS_{}) {{\n        switch (object_kind) {{",
                    upper_snake(invocation.name),
                    upper_snake(failure.status)
                );
                for object in schema
                    .object_kinds
                    .iter()
                    .filter(|object| object.value != 0)
                {
                    let _ = writeln!(
                        output,
                        "        case HYPER_NATIVE_OBJECT_{}:",
                        upper_snake(object.name)
                    );
                }
                let _ = writeln!(
                    output,
                    "            return UINT64_C({mask});\n        default:\n            return UINT64_C(0);\n        }}\n    }}"
                );
            }
        }
    }
    output.push_str("    return UINT64_C(0);\n}\n\n");
}

fn render_c_bit_constants<'a>(
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
        c_doc(output, &format!("{category}: `{name}`."), "");
        let _ = writeln!(
            output,
            "#define {prefix}_{} (UINT64_C(1) << {bit})",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_c_u32_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u32)>,
) {
    for (name, value) in values {
        c_doc(
            output,
            &format!("Object-kind tag `{name}` used with an object-local operation number."),
            "",
        );
        let _ = writeln!(
            output,
            "#define {prefix}_{} UINT32_C({value})",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_c_i64_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, i64)>,
) {
    for (name, value) in values {
        c_doc(
            output,
            &format!(
                "Invocation status `{name}`. Auxiliary results on failure require an explicit method contract."
            ),
            "",
        );
        let rendered = if value < 0 {
            format!("(-INT64_C({}))", value.unsigned_abs())
        } else {
            format!("INT64_C({value})")
        };
        let _ = writeln!(output, "#define {prefix}_{} {rendered}", upper_snake(name));
    }
    output.push('\n');
}

fn c_scalar_field_type(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::I64 => "int64_t",
        FieldKind::U32 => "uint32_t",
        FieldKind::U64 => "uint64_t",
        FieldKind::Bytes(_) => unreachable!("byte arrays require declarator-aware rendering"),
    }
}
