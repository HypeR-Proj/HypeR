// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_abi as generated;

#[test]
fn object_local_operations_decode_and_mask_failures_by_receiver_kind() {
    use generated::NativeOperation;
    assert_eq!(
        generated::HYPER_NATIVE_OP_BYTE_CHANNEL_READ,
        generated::HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE
    );
    let cases = [
        (
            generated::HYPER_NATIVE_OBJECT_BYTE_CHANNEL,
            NativeOperation::ByteChannelRead,
            1,
        ),
        (
            generated::HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL,
            NativeOperation::CapabilityChannelReceive,
            3,
        ),
    ];
    for (kind, expected, mask) in cases {
        let operation = generated::HYPER_NATIVE_OP_BYTE_CHANNEL_READ;
        assert_eq!(NativeOperation::from_raw(kind, operation), Some(expected));
        assert_eq!(expected.receiver_kind(), Some(kind));
        assert_eq!(expected.operation(), operation);
        assert_eq!(
            expected.receiver_rights(),
            generated::HYPER_NATIVE_RIGHT_READ
        );
        assert_eq!(
            generated::hyper_native_failure_result_mask(
                kind,
                operation,
                generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
            ),
            mask
        );
    }
    assert_eq!(NativeOperation::ThreadCreate.argument_count(), 6);
    assert_eq!(NativeOperation::FileReadAt.argument_count(), 4);
    assert_eq!(
        NativeOperation::from_raw(u32::MAX, generated::HYPER_NATIVE_OP_HANDLE_CLOSE),
        None
    );
    assert_eq!(
        NativeOperation::from_raw(generated::HYPER_NATIVE_OBJECT_FILE, u64::MAX),
        None
    );
    assert_eq!(
        NativeOperation::from_raw(
            generated::HYPER_NATIVE_OBJECT_FILE,
            generated::HYPER_NATIVE_OP_HANDLE_CLOSE,
        ),
        Some(NativeOperation::HandleClose)
    );
}

#[test]
fn generated_rust_layouts_are_compiler_checked() {
    assert_eq!(core::mem::size_of::<generated::HyperNativeHandleInfo>(), 16);
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeObjectBasicInfo>(),
        8
    );
    assert_eq!(generated::HYPER_NATIVE_OP_HANDLE_CLOSE, 0);
    assert_eq!(generated::HYPER_NATIVE_RIGHTS_MASK, 0xfff_ffff_ffff);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_EVENT_SIGNALED, 1);
    assert_eq!(generated::HYPER_NATIVE_DEADLINE_INFINITE, u64::MAX);
    assert_eq!(generated::HYPER_NATIVE_HANDLE_INFO_MIN_SIZE, 16);
    assert_eq!(generated::HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE, 32);
    assert_eq!(generated::HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES, 4096);
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeCapabilityDisposition>(),
        24
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeCapabilityDisposition>(),
        8
    );
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeCapabilityReceiveSlot>(),
        24
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeCapabilityReceiveSlot>(),
        8
    );
    assert_eq!(generated::HYPER_NATIVE_OBJECT_BYTE_CHANNEL, 2);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_THREAD, 3);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_PROCESS, 4);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_TASK_GROUP, 5);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN, 6);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_TASK_FACTORY, 7);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY, 8);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_VMO, 9);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_VMAR, 10);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_CONSOLE, 11);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_DIRECTORY, 12);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_FILE, 13);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL, 14);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_PROCESS_BUILDER, 15);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_TASK_INSPECTOR, 16);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR, 17);
    assert_eq!(
        generated::HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY,
        20
    );
    assert_eq!(generated::HYPER_NATIVE_OBJECT_VIRTUAL_CPU, 24);
    assert_eq!(
        generated::hyper_native_object_transfer_class(
            generated::HYPER_NATIVE_OBJECT_PROCESS_BUILDER
        ),
        generated::HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY
    );
    assert_eq!(
        generated::hyper_native_object_transfer_class(
            generated::HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL
        ),
        generated::HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY
    );
    assert_eq!(
        generated::hyper_native_object_transfer_class(u32::MAX),
        generated::HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN
    );
    assert_eq!(generated::HYPER_NATIVE_OP_BYTE_CHANNEL_CREATE, 257);
    assert_eq!(generated::HYPER_NATIVE_OP_BYTE_CHANNEL_WRITE, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_BYTE_CHANNEL_READ, 257);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE, 2);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED, 4);
    assert_eq!(
        generated::HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING,
        1
    );
    assert_eq!(
        generated::HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED,
        2
    );
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_THREAD_TERMINATED, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CONSOLE_READABLE, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE, 2);
    assert_eq!(generated::HYPER_NATIVE_ELF_OSABI, 63);
    assert_eq!(generated::HYPER_NATIVE_ELF_ABI_VERSION, 0);
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeLoaderStartup>(),
        96
    );
    assert_eq!(
        generated::HYPER_NATIVE_PROCESS_STARTUP_DATA_MAX_BYTES,
        16 * 1024
    );
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN,
        1
    );
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP, 2);
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY,
        3
    );
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY,
        4
    );
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR, 5);
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE, 6);
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY,
        7
    );
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeStartupHandle>(),
        16
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeStartupHandle>(),
        8
    );
    assert_eq!(
        generated::HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS,
        u64::MAX
    );
    assert_eq!(generated::HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE, 0);
    assert_eq!(generated::HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE, 1);
    assert_eq!(
        generated::HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES,
        64 * 1024
    );
    assert_eq!(generated::HYPER_NATIVE_FILE_MAX_READ_BYTES, 2 * 1024 * 1024);
    assert_eq!(generated::HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES, 64 * 1024);
    assert_eq!(generated::HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES, 64);
    assert_eq!(generated::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES, 4 * 1024);
    assert_eq!(generated::HYPER_NATIVE_OP_CONSOLE_READ, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_CONSOLE_WRITE, 257);
    assert_eq!(generated::HYPER_NATIVE_OP_DIRECTORY_OPEN_FILE, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_FILE_READ_AT, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_CAPABILITY_CHANNEL_CREATE, 258);
    assert_eq!(generated::HYPER_NATIVE_OP_CAPABILITY_CHANNEL_TRY_SEND, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE, 257);
    assert_eq!(
        core::mem::offset_of!(generated::HyperNativeLoaderStartup, loader_size),
        64
    );
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_CREATE, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_SET_NAME, 256);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_SET_DATA, 257);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_SET_AFFINITY, 258);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_ADD_HANDLE, 259);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_SEAL, 260);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_START, 261);
    assert_eq!(generated::HYPER_NATIVE_OP_PROCESS_BUILDER_ABORT, 262);
    assert_eq!(
        generated::HYPER_NATIVE_OP_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL,
        261
    );
    assert_eq!(generated::HYPER_NATIVE_OP_CLOCK_GET_MONOTONIC, 257);
    assert_eq!(
        core::mem::offset_of!(generated::HyperNativeVirtualMachineInfo, architecture),
        24
    );
    assert_eq!(
        core::mem::offset_of!(generated::HyperNativeVirtualMachineInfo, platform_profile),
        28
    );
    assert_eq!(generated::HYPER_NATIVE_STATUS_WOULD_BLOCK, -13);
    assert_eq!(generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, -14);
    assert_eq!(generated::HYPER_NATIVE_STATUS_PEER_CLOSED, -15);
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_OBJECT_BYTE_CHANNEL,
            generated::HYPER_NATIVE_OP_BYTE_CHANNEL_READ,
            generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
        ),
        0b1
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_OBJECT_BYTE_CHANNEL,
            generated::HYPER_NATIVE_OP_BYTE_CHANNEL_READ,
            generated::HYPER_NATIVE_STATUS_FAULT,
        ),
        0
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL,
            generated::HYPER_NATIVE_OP_CAPABILITY_CHANNEL_RECEIVE,
            generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
        ),
        0b11
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_OBJECT_CONSOLE,
            generated::HYPER_NATIVE_OP_CONSOLE_READ,
            generated::HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ),
        0b1
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_OBJECT_CONSOLE,
            generated::HYPER_NATIVE_OP_CONSOLE_WRITE,
            generated::HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ),
        0b1
    );
}
