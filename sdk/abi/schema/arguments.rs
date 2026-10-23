// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Reusable receiver, argument and result contracts.
//!
//! Argument/result slice order is register order. The receiver travels separately
//! and signed status is not an auxiliary result. Named lengths refer to sibling
//! arguments; memory direction is from the kernel's perspective.
//!
//! Sharing a declaration also shares its rights, bounds and ownership effects.
//! Reuse only when all of those contracts match, not merely when Rust types match.
//! Small helpers below construct metadata; they do not validate live handles.

use super::capabilities::*;
use super::model::*;
use super::values::*;

pub(super) const NO_ARGUMENTS: &[Argument] = &[];
pub(super) const ABI_QUERY_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "revision",
        kind: ValueKind::U64,
        handle: None,
    },
    ResultValue {
        name: "features",
        kind: ValueKind::U64,
        handle: None,
    },
];
pub(super) const CLOCK_GET_MONOTONIC_RESULTS: &[ResultValue] = &[ResultValue {
    name: "nanoseconds",
    kind: ValueKind::U64,
    handle: None,
}];
/// Supported wire-record size, which can exceed the caller's accepted prefix.
/// Individual operations specify which failure statuses also define this result.
pub(super) const INFO_RECORD_RESULTS: &[ResultValue] = &[ResultValue {
    name: "supported_size",
    kind: ValueKind::ByteCount,
    handle: None,
}];
/// Close consumes the receiver itself, with no operation-specific rights needed.
pub(super) const HANDLE_CLOSE_RECEIVER: Argument = Argument {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: 0,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
};
pub(super) const HANDLE_DUPLICATE_RECEIVER: Argument = Argument {
    name: "source",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: RIGHT_DUPLICATE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const HANDLE_DUPLICATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "requested_rights",
    kind: ValueKind::Rights,
    handle: None,
    memory: None,
}];
/// New handle of the source kind with attenuated rights; duplication leaves the
/// source intact, while replacement uses the same result contract but consumes it.
pub(super) const HANDLE_DUPLICATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::SameAsArgument("source"),
        rights: ProducedRights::RequestedSubsetOf("requested_rights"),
    }),
}];
pub(super) const HANDLE_REPLACE_RECEIVER: Argument = Argument {
    name: "source",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: 0,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
};
pub(super) const HANDLE_REPLACE_ARGUMENTS: &[Argument] = &[Argument {
    name: "requested_rights",
    kind: ValueKind::Rights,
    handle: None,
    memory: None,
}];
pub(super) const HANDLE_REPLACE_RESULTS: &[ResultValue] = HANDLE_DUPLICATE_RESULTS;
pub(super) const HANDLE_GET_INFO_RECEIVER: Argument = Argument {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: 0,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const HANDLE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("handle_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const OBJECT_GET_BASIC_INFO_RECEIVER: Argument = Argument {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_GET_BASIC_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("object_basic_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const EXIT_ARGUMENTS: &[Argument] = &[Argument {
    name: "status",
    kind: ValueKind::I64,
    handle: None,
    memory: None,
}];
pub(super) const EVENT_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "options",
    kind: ValueKind::U32,
    handle: None,
    memory: None,
}];
pub(super) const EVENT_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "handle",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("event"),
        rights: ProducedRights::Fixed(EVENT_RIGHTS),
    }),
}];
pub(super) const EVENT_SIGNAL_RECEIVER: Argument = Argument {
    name: "event",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("event"),
        required_rights: RIGHT_SIGNAL,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const EVENT_SIGNAL_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "clear_mask",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "set_mask",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
pub(super) const OBJECT_WAIT_ONE_RECEIVER: Argument = Argument {
    name: "object",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Any,
        required_rights: RIGHT_WAIT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_WAIT_ONE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "signals",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
pub(super) const OBJECT_WAIT_ONE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "observed",
    kind: ValueKind::U64,
    handle: None,
}];

/// The input is an element-counted array of borrowed handles. Every record needs
/// WAIT; the array does not move capabilities or create a persistent subscription.
pub(super) const OBJECT_WAIT_MANY_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "items",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "item_count",
                maximum_elements: OBJECT_WAIT_MANY_MAX_ITEMS,
                element_size: 16,
            },
            record: Some("object_wait_item"),
            handles: Some(IndirectHandles::BorrowRecords {
                handle_field: "handle",
                required_rights: RIGHT_WAIT,
            }),
            validation_order: 0,
        }),
    },
    Argument {
        name: "item_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
];
pub(super) const OBJECT_WAIT_MANY_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "index",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "observed",
        kind: ValueKind::U64,
        handle: None,
    },
];

pub(super) const CHANNEL_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "options",
    kind: ValueKind::U32,
    handle: None,
    memory: None,
}];
pub(super) const CHANNEL_CREATE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "endpoint0",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("byte_channel"),
            rights: ProducedRights::Fixed(BYTE_CHANNEL_RIGHTS),
        }),
    },
    ResultValue {
        name: "endpoint1",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("byte_channel"),
            rights: ProducedRights::Fixed(BYTE_CHANNEL_RIGHTS),
        }),
    },
];
pub(super) const CHANNEL_WRITE_RECEIVER: Argument = Argument {
    name: "endpoint",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("byte_channel"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const CHANNEL_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const CHANNEL_READ_RECEIVER: Argument = Argument {
    name: "endpoint",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("byte_channel"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const CHANNEL_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const CHANNEL_READ_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];
pub(super) const CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS: &[&str] = &["actual_bytes"];
/// A short receive buffer reports the required byte count without consuming the
/// queued message. Other failures leave the auxiliary count undefined.
pub(super) const CHANNEL_READ_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "buffer_too_small",
    results: CHANNEL_READ_BUFFER_TOO_SMALL_RESULTS,
}];

pub(super) const CAPABILITY_CHANNEL_CREATE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "endpoint0",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("capability_channel"),
            rights: ProducedRights::Fixed(CAPABILITY_CHANNEL_RIGHTS),
        }),
    },
    ResultValue {
        name: "endpoint1",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("capability_channel"),
            rights: ProducedRights::Fixed(CAPABILITY_CHANNEL_RIGHTS),
        }),
    },
];

pub(super) const CAPABILITY_CHANNEL_SEND_RECEIVER: Argument = Argument {
    name: "endpoint",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("capability_channel"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Message bytes precede capability dispositions in user-memory validation.
/// Each disposition selects move/duplicate; the entire capability transaction
/// commits only on OK, so failed sends do not consume source handles.
pub(super) const CAPABILITY_CHANNEL_SEND_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "dispositions",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "disposition_count",
                maximum_elements: 16,
                element_size: 24,
            },
            record: Some("capability_disposition"),
            handles: Some(IndirectHandles::ConsumeRecords {
                handle_field: "handle",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                operation_field: "operation",
                common_rights: RIGHT_TRANSFER,
                operations: CAPABILITY_OPERATIONS,
                commit: CapabilityCommit::AtomicOnOk,
            }),
            validation_order: 1,
        }),
    },
    Argument {
        name: "disposition_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const CAPABILITY_CHANNEL_RECEIVE_RECEIVER: Argument = Argument {
    name: "endpoint",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("capability_channel"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Receive slots are input constraints as well as output locations. The caller
/// declares exact kind/rights before transferred handles may be published.
/// Copied bytes on FAULT are not published capabilities.
pub(super) const CAPABILITY_CHANNEL_RECEIVE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "deadline",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: 4 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "capability_slots",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::ReadWrite,
            length: MemoryLength::Elements {
                argument: "slot_count",
                maximum_elements: 16,
                element_size: 24,
            },
            record: Some("capability_receive_slot"),
            handles: Some(IndirectHandles::ProduceTransferred {
                handle_field: "handle",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                flags_field: "flags",
                commit: CapabilityCommit::AtomicOnOk,
            }),
            validation_order: 1,
        }),
    },
    Argument {
        name: "slot_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];
pub(super) const CAPABILITY_CHANNEL_RECEIVE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "actual_bytes",
        kind: ValueKind::ByteCount,
        handle: None,
    },
    ResultValue {
        name: "actual_capabilities",
        kind: ValueKind::ElementCount,
        handle: None,
    },
];
pub(super) const CAPABILITY_CHANNEL_RECEIVE_BUFFER_TOO_SMALL_RESULTS: &[&str] =
    &["actual_bytes", "actual_capabilities"];
pub(super) const CAPABILITY_CHANNEL_RECEIVE_FAILURE_RESULTS: &[FailureResults] =
    &[FailureResults {
        status: "buffer_too_small",
        results: CAPABILITY_CHANNEL_RECEIVE_BUFFER_TOO_SMALL_RESULTS,
    }];

pub(super) const CONSOLE_READ_RECEIVER: Argument = Argument {
    name: "console",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("console"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const CONSOLE_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_capacity",
                maximum_bytes: CONSOLE_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const CONSOLE_WRITE_RECEIVER: Argument = Argument {
    name: "console",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("console"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const CONSOLE_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: CONSOLE_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const CONSOLE_IO_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];
pub(super) const CONSOLE_IO_WOULD_BLOCK_RESULTS: &[&str] = &["actual_bytes"];
pub(super) const CONSOLE_IO_FAILURE_RESULTS: &[FailureResults] = &[FailureResults {
    status: "would_block",
    results: CONSOLE_IO_WOULD_BLOCK_RESULTS,
}];

pub(super) const DIRECTORY_OPEN_FILE_RECEIVER: Argument = Argument {
    name: "directory",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("directory"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const DIRECTORY_OPEN_FILE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "path",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "path_size",
                maximum_bytes: 4096,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "path_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "requested_rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];
pub(super) const DIRECTORY_OPEN_FILE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "file",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("file"),
        rights: ProducedRights::ExactRequested {
            argument: "requested_rights",
            allowed_rights: FILE_RIGHTS,
            authority_source: Some("directory"),
        },
    }),
}];

pub(super) const DIRECTORY_OPEN_DIRECTORY_RESULTS: &[ResultValue] = &[ResultValue {
    name: "child",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("directory"),
        rights: ProducedRights::ExactRequested {
            argument: "requested_rights",
            allowed_rights: DIRECTORY_RIGHTS,
            authority_source: Some("directory"),
        },
    }),
}];

pub(super) const DIRECTORY_READ_RECEIVER: Argument = Argument {
    name: "stream",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("directory_stream"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Bounded page of directory entries. The cookie is opaque continuation state,
/// not a byte offset; capacity counts records rather than bytes.
pub(super) const DIRECTORY_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "cookie",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: DIRECTORY_ENTRY_PAGE_CAPACITY,
                element_size: DIRECTORY_ENTRY_RECORD_SIZE,
            },
            record: Some("directory_entry"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];

pub(super) const DIRECTORY_READ_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "count",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "next_cookie",
        kind: ValueKind::U64,
        handle: None,
    },
];

pub(super) const FILE_GET_INFO_RECEIVER: Argument = Argument {
    name: "file",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("file"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const FILE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("file_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const DIRECTORY_GET_INFO_RECEIVER: Argument = Argument {
    name: "directory",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("directory"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const DIRECTORY_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("directory_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const VMO_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "size",
    kind: ValueKind::ByteCount,
    handle: None,
    memory: None,
}];
pub(super) const VMO_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("vmo"),
        rights: ProducedRights::Fixed(VMO_WRITABLE_RIGHTS),
    }),
}];

pub(super) const SNAPSHOT_VMO_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("vmo"),
            rights: ProducedRights::Fixed(VMO_EXECUTABLE_RIGHTS & !RIGHT_EXECUTE),
        }),
    },
    scalar_result("byte_size", ValueKind::ByteCount),
];

pub(super) const VMO_READ_RECEIVER: Argument = Argument {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmo"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VMO_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const VMO_WRITE_RECEIVER: Argument = Argument {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmo"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VMO_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: 64 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const VMAR_ALLOCATE_RECEIVER: Argument = vmar_argument(RIGHT_MAP);
pub(super) const VMAR_ALLOCATE_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
    scalar_argument("options", ValueKind::U64),
];
pub(super) const VMAR_ALLOCATE_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "child",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("vmar"),
            rights: ProducedRights::FixedAndInherited {
                fixed_rights: VMAR_RIGHTS,
                authority_source: "vmar",
                inherited_rights: RIGHT_LOCK_MEMORY,
            },
        }),
    },
    ResultValue {
        name: "address",
        kind: ValueKind::U64,
        handle: None,
    },
];

pub(super) const VMAR_MAP_RECEIVER: Argument = vmar_argument(RIGHT_MAP);
pub(super) const VMAR_MAP_ARGUMENTS: &[Argument] = &[
    vmo_argument(RIGHT_MAP),
    scalar_argument("vmo_offset", ValueKind::U64),
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
    scalar_argument("permissions", ValueKind::U32),
];

pub(super) const VMAR_RANGE_RECEIVER: Argument = vmar_argument(RIGHT_MAP);
pub(super) const VMAR_RANGE_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
    scalar_argument("permissions", ValueKind::U32),
];

pub(super) const VMAR_UNMAP_RECEIVER: Argument = vmar_argument(RIGHT_MAP);
pub(super) const VMAR_UNMAP_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("size", ValueKind::ByteCount),
];

pub(super) const VMAR_DESTROY_RECEIVER: Argument = Argument {
    name: "vmar",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmar"),
        required_rights: RIGHT_MAP,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
};

pub(super) const FILE_READ_AT_RECEIVER: Argument = Argument {
    name: "file",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("file"),
        required_rights: RIGHT_READ,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const FILE_READ_AT_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "options",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "offset",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_capacity",
                maximum_bytes: 2 * 1024 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "output_capacity",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const FILE_WRITE_AT_RECEIVER: Argument = file_argument(RIGHT_WRITE);
pub(super) const FILE_WRITE_AT_ARGUMENTS: &[Argument] = &[
    scalar_argument("options", ValueKind::U32),
    scalar_argument("offset", ValueKind::U64),
    Argument {
        name: "input",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "input_size",
                maximum_bytes: 2 * 1024 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("input_size", ValueKind::ByteCount),
];
pub(super) const fn wait_set_argument(rights: u64) -> Argument {
    Argument {
        name: "wait_set",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("wait_set"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

pub(super) const fn directory_argument(rights: u64) -> Argument {
    Argument {
        name: "directory",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("directory"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

pub(super) const fn file_argument(rights: u64) -> Argument {
    Argument {
        name: "file",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

pub(super) const FILE_READ_AT_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "actual_bytes",
        kind: ValueKind::ByteCount,
        handle: None,
    },
    ResultValue {
        name: "file_size",
        kind: ValueKind::ByteCount,
        handle: None,
    },
];

pub(super) const PROCESS_BUILDER_CREATE_RECEIVER: Argument = Argument {
    name: "factory",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_factory"),
        required_rights: RIGHT_CREATE_PROCESS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const PROCESS_BUILDER_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "group",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("task_group"),
            required_rights: RIGHT_TASK_GROUP_ATTACH_PROCESS,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "domain",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("resource_domain"),
            required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "executable",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: RIGHT_READ
                | RIGHT_EXECUTE
                | RIGHT_PROTECT
                | RIGHT_DUPLICATE
                | RIGHT_TRANSFER,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
    Argument {
        name: "loader",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("file"),
            required_rights: RIGHT_READ | RIGHT_EXECUTE,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    },
];
pub(super) const PROCESS_BUILDER_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "builder",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("process_builder"),
        rights: ProducedRights::FixedAndInherited {
            fixed_rights: PROCESS_BUILDER_RIGHTS,
            authority_source: "factory",
            inherited_rights: RIGHT_LOCK_MEMORY
                | RIGHT_RETAIN_PROCESS
                | SCHEDULING_ELEVATION_RIGHTS,
        },
    }),
}];

pub(super) const PROCESS_BUILDER_SET_NAME_RECEIVER: Argument =
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PROCESS_BUILDER_SET_NAME_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "name",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "name_size",
                maximum_bytes: 64,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "name_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const PROCESS_BUILDER_SET_DATA_RECEIVER: Argument =
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PROCESS_BUILDER_SET_DATA_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "data",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "data_size",
                maximum_bytes: 16 * 1024,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "data_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const PROCESS_BUILDER_SET_AFFINITY_RECEIVER: Argument =
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PROCESS_BUILDER_SET_AFFINITY_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "affinity_words",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Elements {
                argument: "word_count",
                maximum_elements: 4,
                element_size: 8,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "word_count",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const PROCESS_BUILDER_ADD_HANDLE_RECEIVER: Argument =
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PROCESS_BUILDER_ADD_HANDLE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "source",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Any,
            required_rights: RIGHT_TRANSFER,
            disposition: HandleDisposition::ByOperation {
                argument: "operation",
                operations: CAPABILITY_OPERATIONS,
            },
        }),
        memory: None,
    },
    Argument {
        name: "purpose",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "expected_kind",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
    Argument {
        name: "rights",
        kind: ValueKind::Rights,
        handle: None,
        memory: None,
    },
    Argument {
        name: "operation",
        kind: ValueKind::U32,
        handle: None,
        memory: None,
    },
];

pub(super) const PROCESS_BUILDER_SEAL_RECEIVER: Argument =
    process_builder_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PROCESS_BUILDER_START_RECEIVER: Argument =
    process_builder_argument(RIGHT_START, HandleDisposition::ConsumeOnCommit);
pub(super) const PROCESS_BUILDER_START_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "process",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("process"),
            rights: ProducedRights::Fixed(PROCESS_SUPERVISOR_RIGHTS),
        }),
    },
    ResultValue {
        name: "startup_channel",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("byte_channel"),
            rights: ProducedRights::Fixed(RIGHT_READ | RIGHT_WAIT),
        }),
    },
];
pub(super) const PROCESS_BUILDER_ABORT_RECEIVER: Argument =
    process_builder_argument(RIGHT_REQUEST_STOP, HandleDisposition::ConsumeOnCommit);

pub(super) const PROCESS_REQUEST_STOP_RECEIVER: Argument = Argument {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("process"),
        required_rights: RIGHT_REQUEST_STOP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};

pub(super) const PROCESS_GET_INFO_RECEIVER: Argument = Argument {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("process"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const PROCESS_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("process_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "info_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const TASK_INSPECTOR_SCAN_PROCESSES_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_INSPECTOR_SCAN_PROCESSES_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 96,
            },
            record: Some("task_process"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const TASK_INSPECTOR_SCAN_THREADS_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_INSPECTOR_SCAN_THREADS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 104,
            },
            record: Some("task_thread"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const OBJECT_INSPECTOR_SCAN_OBJECTS_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_SCAN_OBJECTS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 104,
            },
            record: Some("object_inspection"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const OBJECT_INSPECTOR_SCAN_HANDLES_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_SCAN_HANDLES_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "process_koid",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "records",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Elements {
                argument: "capacity",
                maximum_elements: 8,
                element_size: 40,
            },
            record: Some("handle_inspection"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "capacity",
        kind: ValueKind::ElementCount,
        handle: None,
        memory: None,
    },
];

pub(super) const OBJECT_INSPECTOR_READ_DETAILS_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: RIGHT_INSPECT | RIGHT_INSPECT_DETAILS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_READ_DETAILS_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "process_koid",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "target",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "cursor",
        kind: ValueKind::U64,
        handle: None,
        memory: None,
    },
    Argument {
        name: "record",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "record_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("object_details"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "record_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const INSPECTOR_SCAN_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "count",
        kind: ValueKind::ElementCount,
        handle: None,
    },
    ResultValue {
        name: "next_cursor",
        kind: ValueKind::U64,
        handle: None,
    },
];

pub(super) const TASK_INSPECTOR_DERIVE_PROCESS_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_inspector"),
        required_rights: TASK_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_INSPECTOR_DERIVE_PROCESS_ARGUMENTS: &[Argument] = &[Argument {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("process"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const TASK_INSPECTOR_DERIVE_PROCESS_RESULTS: &[ResultValue] = &[ResultValue {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("task_inspector"),
        rights: ProducedRights::Fixed(TASK_INSPECTOR_RIGHTS),
    }),
}];

pub(super) const OBJECT_INSPECTOR_DERIVE_PROCESS_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: OBJECT_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_DERIVE_PROCESS_ARGUMENTS: &[Argument] = &[Argument {
    name: "process",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("process"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const OBJECT_INSPECTOR_DERIVE_PROCESS_RESULTS: &[ResultValue] = &[ResultValue {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("object_inspector"),
        rights: ProducedRights::Fixed(OBJECT_INSPECTOR_RIGHTS),
    }),
}];

pub(super) const MEMORY_INSPECTOR_READ_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("memory_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const MEMORY_INSPECTOR_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "observation",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "observation_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("memory_observation"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "observation_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const CPU_INSPECTOR_READ_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("cpu_inspector"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const CPU_INSPECTOR_READ_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "observation",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "observation_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("cpu_observation"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "observation_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const TASK_INSPECTOR_DERIVE_TASK_GROUP_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_inspector"),
        required_rights: TASK_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS: &[Argument] = &[Argument {
    name: "task_group",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_group"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const OBJECT_INSPECTOR_DERIVE_TASK_GROUP_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: OBJECT_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_DERIVE_TASK_GROUP_ARGUMENTS: &[Argument] = &[Argument {
    name: "task_group",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_group"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_inspector"),
        required_rights: TASK_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS: &[Argument] = &[Argument {
    name: "resource_domain",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("resource_domain"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN_RECEIVER: Argument = Argument {
    name: "inspector",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("object_inspector"),
        required_rights: OBJECT_INSPECTOR_RIGHTS,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN_ARGUMENTS: &[Argument] = &[Argument {
    name: "resource_domain",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("resource_domain"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

/// Describe a builder input, making successful consumption explicit at each
/// call site instead of inferring it from the operation name.
pub(super) const fn process_builder_argument(
    required_rights: u64,
    disposition: HandleDisposition,
) -> Argument {
    Argument {
        name: "builder",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("process_builder"),
            required_rights,
            disposition,
        }),
        memory: None,
    }
}

pub(super) const fn scalar_result(name: &'static str, kind: ValueKind) -> ResultValue {
    ResultValue {
        name,
        kind,
        handle: None,
    }
}

/// Metadata for a register with no handle admission or user-memory access.
/// This helper establishes no safety for dereferencing an address or operating
/// on a handle; any special interpretation needs a separate method contract.
pub(super) const fn scalar_argument(name: &'static str, kind: ValueKind) -> Argument {
    Argument {
        name,
        kind,
        handle: None,
        memory: None,
    }
}

pub(super) const fn vmar_argument(required_rights: u64) -> Argument {
    Argument {
        name: "vmar",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmar"),
            required_rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

pub(super) const fn vmo_argument(required_rights: u64) -> Argument {
    Argument {
        name: "vmo",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("vmo"),
            required_rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

pub(super) const VIRTUAL_MACHINE_CREATION_LEASE_CREATE_RECEIVER: Argument = Argument {
    name: "authority",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_machine_creation_authority"),
        required_rights: RIGHT_DERIVE | RIGHT_CREATE_VIRTUAL_MACHINE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VIRTUAL_MACHINE_CREATION_LEASE_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "resource_domain",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("resource_domain"),
        required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];
pub(super) const VIRTUAL_MACHINE_CREATION_LEASE_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "lease",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("virtual_machine_creation_lease"),
        rights: ProducedRights::FixedAndInherited {
            fixed_rights: VIRTUAL_MACHINE_CREATION_LEASE_RIGHTS,
            authority_source: "authority",
            inherited_rights: SCHEDULING_RIGHTS,
        },
    }),
}];

pub(super) const RESOURCE_DOMAIN_CREATE_RECEIVER: Argument = Argument {
    name: "parent",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("resource_domain"),
        required_rights: RIGHT_CREATE_RESOURCE_DOMAIN,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const RESOURCE_DOMAIN_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "limits",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "limits_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("resource_limits"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("limits_size", ValueKind::ByteCount),
];
pub(super) const RESOURCE_DOMAIN_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "child",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("resource_domain"),
        rights: ProducedRights::Fixed(
            RIGHT_DUPLICATE
                | RIGHT_TRANSFER
                | RIGHT_INSPECT
                | RIGHT_CREATE_RESOURCE_DOMAIN
                | RIGHT_SET_LIMITS
                | RIGHT_REVOKE
                | RIGHT_RESOURCE_DOMAIN_SPONSOR,
        ),
    }),
}];

pub(super) const TASK_GROUP_CREATE_RECEIVER: Argument = Argument {
    name: "factory",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("task_factory"),
        required_rights: RIGHT_CREATE_TASK_GROUP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const TASK_GROUP_CREATE_ARGUMENTS: &[Argument] = &[Argument {
    name: "resource_domain",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("resource_domain"),
        required_rights: RIGHT_RESOURCE_DOMAIN_SPONSOR,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];
pub(super) const TASK_GROUP_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "group",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("task_group"),
        rights: ProducedRights::Fixed(
            RIGHT_DUPLICATE
                | RIGHT_TRANSFER
                | RIGHT_INSPECT
                | RIGHT_REQUEST_STOP
                | RIGHT_TASK_GROUP_ATTACH_PROCESS
                | RIGHT_WAIT,
        ),
    }),
}];

/// The creation lease is single-use: consume only when a pending VM is published.
pub(super) const VIRTUAL_MACHINE_CREATE_RECEIVER: Argument = Argument {
    name: "lease",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_machine_creation_lease"),
        required_rights: RIGHT_CREATE_VIRTUAL_MACHINE,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
};
pub(super) const VIRTUAL_MACHINE_CREATE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "configuration",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "configuration_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_machine_configuration"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "configuration_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];
pub(super) const VIRTUAL_MACHINE_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "pending_virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("pending_virtual_machine"),
        rights: ProducedRights::FixedAndInherited {
            fixed_rights: PENDING_VIRTUAL_MACHINE_RIGHTS,
            authority_source: "lease",
            inherited_rights: SCHEDULING_RIGHTS,
        },
    }),
}];

pub(super) const PENDING_VIRTUAL_MACHINE_SET_MEMORY_RECEIVER: Argument = Argument {
    name: "pending_virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("pending_virtual_machine"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Borrow the backing VMO handle; the VM retains its own backing ownership.
/// Hardware-write exclusion is a separate lifetime rule from handle consumption.
pub(super) const PENDING_VIRTUAL_MACHINE_SET_MEMORY_ARGUMENTS: &[Argument] = &[Argument {
    name: "vmo",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmo"),
        required_rights: RIGHT_READ | RIGHT_WRITE | RIGHT_MAP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL_RECEIVER: Argument = Argument {
    name: "pending_virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("pending_virtual_machine"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Successful serial binding consumes the route handle; failed binding preserves it.
pub(super) const PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL_ARGUMENTS: &[Argument] = &[Argument {
    name: "virtual_serial",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_serial"),
        required_rights: RIGHT_TRANSFER | RIGHT_ASSIGN_DEVICE,
        disposition: HandleDisposition::ConsumeOnCommit,
    }),
    memory: None,
}];

pub(super) const VIRTUAL_SERIAL_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "virtual_serial",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("virtual_serial"),
        rights: ProducedRights::Fixed(VIRTUAL_SERIAL_RIGHTS),
    }),
}];

pub(super) const VIRTUAL_SERIAL_REGISTER_OUTPUT_RECEIVER: Argument = Argument {
    name: "virtual_serial",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_serial"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
/// Borrow an exact-size output VMO; successful registration retains an exclusive
/// writer lease until port retirement. Read-only consumer mappings may coexist.
pub(super) const VIRTUAL_SERIAL_REGISTER_OUTPUT_ARGUMENTS: &[Argument] = &[Argument {
    name: "buffer",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("vmo"),
        required_rights: RIGHT_READ | RIGHT_WRITE | RIGHT_MAP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
}];

pub(super) const VIRTUAL_SERIAL_WRITE_RECEIVER: Argument = Argument {
    name: "virtual_serial",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_serial"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VIRTUAL_SERIAL_WRITE_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "bytes",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "byte_count",
                maximum_bytes: VIRTUAL_SERIAL_MAX_TRANSFER_BYTES,
            },
            record: None,
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "byte_count",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const VIRTUAL_SERIAL_IO_RESULTS: &[ResultValue] = &[ResultValue {
    name: "actual_bytes",
    kind: ValueKind::ByteCount,
    handle: None,
}];

pub(super) const PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP_RECEIVER: Argument = Argument {
    name: "pending_virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("pending_virtual_machine"),
        required_rights: RIGHT_WRITE,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "bootstrap",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Read,
            length: MemoryLength::Bytes {
                argument: "bootstrap_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_cpu_bootstrap"),
            handles: None,
            validation_order: 0,
        }),
    },
    Argument {
        name: "bootstrap_size",
        kind: ValueKind::ByteCount,
        handle: None,
        memory: None,
    },
];

pub(super) const fn pending_virtual_machine_argument(
    required_rights: u64,
    disposition: HandleDisposition,
) -> Argument {
    Argument {
        name: "pending_virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("pending_virtual_machine"),
            required_rights,
            disposition,
        }),
        memory: None,
    }
}

pub(super) const PENDING_VIRTUAL_MACHINE_SEAL_RECEIVER: Argument =
    pending_virtual_machine_argument(RIGHT_WRITE, HandleDisposition::Borrow);
pub(super) const PENDING_VIRTUAL_MACHINE_INSTALL_RECEIVER: Argument =
    pending_virtual_machine_argument(RIGHT_START, HandleDisposition::ConsumeOnCommit);
/// Publish the installed VM and dormant boot vCPU together. Installation consumes
/// the pending VM on success but does not start the returned vCPU.
pub(super) const PENDING_VIRTUAL_MACHINE_INSTALL_RESULTS: &[ResultValue] = &[
    ResultValue {
        name: "virtual_machine",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("virtual_machine"),
            rights: ProducedRights::FixedAndInherited {
                fixed_rights: VIRTUAL_MACHINE_RIGHTS,
                authority_source: "pending_virtual_machine",
                inherited_rights: SCHEDULING_RIGHTS,
            },
        }),
    },
    ResultValue {
        name: "boot_virtual_cpu",
        kind: ValueKind::Handle,
        handle: Some(ProducedHandle {
            object: ProducedObject::Kind("virtual_cpu"),
            rights: ProducedRights::FixedAndInherited {
                fixed_rights: VIRTUAL_CPU_RIGHTS,
                authority_source: "pending_virtual_machine",
                inherited_rights: SCHEDULING_ELEVATION_RIGHTS,
            },
        }),
    },
];
pub(super) const PENDING_VIRTUAL_MACHINE_ABORT_RECEIVER: Argument =
    pending_virtual_machine_argument(RIGHT_REQUEST_STOP, HandleDisposition::ConsumeOnCommit);

pub(super) const VIRTUAL_MACHINE_REQUEST_STOP_RECEIVER: Argument = Argument {
    name: "virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_machine"),
        required_rights: RIGHT_REQUEST_STOP,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};

pub(super) const VIRTUAL_MACHINE_GET_INFO_RECEIVER: Argument = Argument {
    name: "virtual_machine",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_machine"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VIRTUAL_MACHINE_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_machine_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("info_size", ValueKind::ByteCount),
];

pub(super) const VIRTUAL_CPU_GET_INFO_RECEIVER: Argument = Argument {
    name: "virtual_cpu",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_cpu"),
        required_rights: RIGHT_INSPECT,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};
pub(super) const VIRTUAL_CPU_GET_INFO_ARGUMENTS: &[Argument] = &[
    Argument {
        name: "info",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "info_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some("virtual_cpu_info"),
            handles: None,
            validation_order: 0,
        }),
    },
    scalar_argument("info_size", ValueKind::ByteCount),
];

pub(super) const VIRTUAL_CPU_START_RECEIVER: Argument = Argument {
    name: "virtual_cpu",
    kind: ValueKind::Handle,
    handle: Some(HandleArgument {
        object: ObjectConstraint::Kind("virtual_cpu"),
        required_rights: RIGHT_START,
        disposition: HandleDisposition::Borrow,
    }),
    memory: None,
};

pub(super) const THREAD_CREATE_ARGUMENTS: &[Argument] = &[
    scalar_argument("entry", ValueKind::U64),
    scalar_argument("stack", ValueKind::U64),
    scalar_argument("tls", ValueKind::U64),
    scalar_argument("argument", ValueKind::U64),
    PROCESS_BUILDER_SET_AFFINITY_ARGUMENTS[0],
    PROCESS_BUILDER_SET_AFFINITY_ARGUMENTS[1],
];
pub(super) const THREAD_CREATE_RESULTS: &[ResultValue] = &[ResultValue {
    name: "thread",
    kind: ValueKind::Handle,
    handle: Some(ProducedHandle {
        object: ProducedObject::Kind("thread"),
        rights: ProducedRights::FixedAndInherited {
            fixed_rights: RIGHT_DUPLICATE
                | RIGHT_TRANSFER
                | RIGHT_WAIT
                | RIGHT_INSPECT
                | RIGHT_START
                | RIGHT_REQUEST_STOP
                | RIGHT_SCHED_LOWER,
            authority_source: "receiver",
            inherited_rights: SCHEDULING_ELEVATION_RIGHTS,
        },
    }),
}];
pub(super) const fn thread_argument(rights: u64) -> Argument {
    Argument {
        name: "thread",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind("thread"),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}
/// Compare-before-sleep on a caller address with an absolute monotonic deadline;
/// the scalar address is handled by the dedicated atomic-wait path, not byte copy.
pub(super) const ATOMIC_WAIT_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("expected", ValueKind::U32),
    scalar_argument("deadline", ValueKind::U64),
];
pub(super) const ATOMIC_WAKE_ARGUMENTS: &[Argument] = &[
    scalar_argument("address", ValueKind::U64),
    scalar_argument("count", ValueKind::U32),
];

pub(super) const fn retained_object_argument(object: &'static str, rights: u64) -> Argument {
    Argument {
        name: "object",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind(object),
            required_rights: rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

/// Bounded extensible output for retained-state inspection; no live capability
/// is published through the returned diagnostic record.
pub(super) const fn crash_output_argument(record: &'static str) -> Argument {
    Argument {
        name: "output",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction: MemoryDirection::Write,
            length: MemoryLength::Bytes {
                argument: "output_size",
                maximum_bytes: EXTENSIBLE_RECORD_MAX_BYTES,
            },
            record: Some(record),
            handles: None,
            validation_order: 0,
        }),
    }
}

/// Scheduling setters inspect each changed field against the receiver rights.
/// A zero static receiver mask never grants permission to change configuration.
const fn scheduling_record(direction: MemoryDirection) -> Argument {
    Argument {
        name: "configuration",
        kind: ValueKind::UserAddress,
        handle: None,
        memory: Some(UserMemory {
            direction,
            length: MemoryLength::FixedBytes { bytes: 64 },
            record: Some("scheduling_parameters"),
            handles: None,
            validation_order: 0,
        }),
    }
}
pub(super) const SCHEDULING_INPUT_ARGUMENTS: &[Argument] =
    &[scheduling_record(MemoryDirection::Read)];
pub(super) const SCHEDULING_OUTPUT_ARGUMENTS: &[Argument] =
    &[scheduling_record(MemoryDirection::Write)];
pub(super) const SCHEDULING_BUILDER_ARGUMENTS: &[Argument] = &[
    scheduling_record(MemoryDirection::Read),
    scalar_argument("delegated_rights", ValueKind::Rights),
];
