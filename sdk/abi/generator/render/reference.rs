// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic reference output for a validated ABI schema.

use super::super::schema;
use super::constant_group_kind;
use super::documentation::{
    field_kind_name, invocation_documentation, prose_markdown, transfer_class_name,
};
use schema::AbiSchema;
use std::fmt::Write as _;

pub(in super::super) fn render_reference(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "<!--\n\
         SPDX-FileCopyrightText: 2026 roolrz\n\
         SPDX-License-Identifier: Apache-2.0\n\
         -->\n\n\
         # HypeR Native ABI reference\n\n\
         This file is generated from `schema/native.rs`. Do not edit it directly.\n\n",
    );
    let _ = writeln!(output, "ABI revision: `{}`.\n", schema.revision);
    output.push_str("## Status values\n\n| Value | Name |\n| ---: | --- |\n");
    for status in schema.statuses {
        let _ = writeln!(output, "| {} | `{}` |", status.value, status.name);
    }
    output.push('\n');
    output.push_str("## Object kinds\n\n| Value | Name | Transfer |\n| ---: | --- | --- |\n");
    for object in schema.object_kinds {
        let _ = writeln!(
            output,
            "| {} | `{}` | `{}` |",
            object.value,
            object.name,
            transfer_class_name(object.transfer)
        );
    }
    output.push('\n');
    output.push_str("## Object signals\n\n| Object | Bit | Name |\n| --- | ---: | --- |\n");
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "| `{}` | {} | `{}` |",
            signal.object, signal.bit, signal.name
        );
    }
    output.push('\n');
    output.push_str(
        "## Scalar domains\n\n\
        Enum domains contain mutually exclusive values; flags contain independent bits.\n\
        Constants contain bounds, sizes, addresses and sentinels. All are exported as\n\
        fixed-width integer constants; grouping does not change wire field types.\n\n",
    );
    for group in schema.constant_groups {
        let _ = writeln!(
            output,
            "### `{}` ({})\n\n{}\n\n| Name | Value | Meaning |\n| --- | ---: | --- |",
            group.name,
            constant_group_kind(group.kind),
            group.description.trim()
        );
        for constant in group.values {
            let _ = writeln!(
                output,
                "| `{}` | `{}` | {} |",
                constant.name,
                constant.value,
                constant
                    .description
                    .trim()
                    .replace('|', "\\|")
                    .replace('\n', " ")
            );
        }
        output.push('\n');
    }
    output.push_str("## Semantic rules\n\n");
    for rule in schema.semantic_rules {
        let _ = writeln!(output, "- {}", prose_markdown(rule));
    }
    output.push('\n');
    output.push_str(
        "## Invocations\n\n\
         Auxiliary result registers are defined only for `ok` unless a result is annotated with\n\
         `also-on=<status>`. Element-count memory ranges are checked as count times the declared\n\
         element size before any user-memory access.\n\n",
    );
    for invocation in schema.invocations {
        let _ = writeln!(output, "### `{}`\n", invocation.name);
        output.push_str(&invocation_documentation(invocation));
        output.push('\n');
    }
    output.push_str("## Public records\n\n");
    for record in schema.records {
        let _ = writeln!(
            output,
            "### `{}`\n\n\
             Minimum prefix: {} bytes; size: {} bytes; alignment: {} bytes.\n\n\
             | Offset | Field | Type |\n\
             | ---: | --- | --- |",
            record.name, record.minimum_size, record.size, record.alignment
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "| {} | `{}` | `{}` |",
                field.offset,
                field.name,
                field_kind_name(field.kind)
            );
        }
        output.push('\n');
    }
    if output.ends_with("\n\n") {
        output.pop();
    }
    output
}
