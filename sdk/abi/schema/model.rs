// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Schema vocabulary, layouts and invocation contracts.
//!
//! These types describe the ABI; they are not its wire representations.
//! [`Record`] and [`FieldKind`] determine generated C/Rust layouts, while
//! [`Invocation`] describes registers and capability effects. References by
//! name are resolved by the host-side generator's validation pass.
//!
//! A numeric address is not permission to access memory, and a numeric handle
//! is not permission to operate on an object: [`UserMemory`] and
//! [`HandleArgument`] carry those additional contracts.

/// Single source of wire values, layout constraints and capability effects.
/// Renderers consume this metadata; none of these Rust types cross the ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbiSchema {
    pub revision: u64,
    pub features: &'static [Feature],
    pub statuses: &'static [Status],
    pub object_kinds: &'static [ObjectKind],
    pub rights: &'static [Right],
    pub signals: &'static [Signal],
    pub constant_groups: &'static [ConstantGroup],
    pub records: &'static [Record],
    pub invocations: &'static [Invocation],
    /// Cross-operation invariants not expressible by a single layout or signature.
    pub semantic_rules: &'static [&'static str],
}

/// Signed status returned separately from auxiliary result registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    /// Zero is success; failures are negative. These are not host errno values.
    pub value: i64,
    pub name: &'static str,
}

/// An advertised feature bit, not a Cargo build-feature identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Feature {
    /// Bit position, not a pre-shifted mask; must be less than 64.
    pub bit: u8,
    pub name: &'static str,
}

/// Stable object tag used to decode an object-local operation number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectKind {
    /// Zero is reserved for `none`, never a valid invocation receiver.
    pub value: u32,
    pub name: &'static str,
    pub transfer: TransferClass,
}

/// Permitted transfer mechanism; TRANSFER authority is still required.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferClass {
    /// May not be exported through capability transport.
    Forbidden,
    /// Buffered transfer, rendezvous and staged startup are admitted.
    General,
    /// Rendezvous and staged startup only; no buffered capability queue.
    RendezvousOnly,
}

/// Capability-right bit. Meaning is constrained by the receiving object kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Right {
    /// Position in the u64 rights mask, not the mask itself.
    pub bit: u8,
    pub name: &'static str,
}

/// Object-local wait bit; equal bit positions on different kinds are unrelated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signal {
    /// Name of the object kind that owns this signal namespace.
    pub object: &'static str,
    pub bit: u8,
    pub name: &'static str,
}

/// Named scalar exported as an integer constant, not as a Rust enum variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbiConstant {
    pub name: &'static str,
    pub value: u64,
    /// Optional member-specific Markdown, in addition to its domain description.
    pub description: &'static str,
}

/// A named scalar domain. Values remain fixed-width integers on the wire;
/// grouping does not turn untrusted integer fields into Rust enum values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstantGroup {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: ConstantGroupKind,
    pub values: &'static [AbiConstant],
}

/// Validation rule for a scalar domain; it does not change the wire integer type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantGroupKind {
    /// Mutually exclusive named values; discriminants must be unique.
    Enumeration,
    /// Independently combinable bits; each member is a distinct single bit.
    Flags,
    /// Limits, sizes, addresses and sentinels; repeated values are valid.
    Constants,
}

impl AbiSchema {
    /// Flatten domains in declaration order without allocating or renumbering.
    pub fn constants(&self) -> impl Iterator<Item = &AbiConstant> {
        self.constant_groups.iter().flat_map(|group| group.values)
    }
}

/// Explicit wire layout, independent of the generator host's pointer width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub name: &'static str,
    /// Members in increasing, nonoverlapping byte-offset order.
    pub fields: &'static [Field],
    /// Smallest prefix which preserves this record's original ABI contract.
    ///
    /// Future extensions may increase `size`, but must not increase this value
    /// while the original prefix remains supported.
    pub minimum_size: u16,
    /// Complete current size in bytes, including explicit and trailing padding.
    pub size: u16,
    /// Required alignment in bytes; must agree with the fields' natural alignment.
    pub alignment: u8,
}

/// Member position in bytes from the start of its wire record.
/// Gaps are emitted as explicit padding, never inferred from a host Rust layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: &'static str,
    pub kind: FieldKind,
    /// Byte offset from the start of the record, never a host pointer or index.
    pub offset: u16,
}

/// Fixed-width wire storage. Pointer-sized integers and native Rust enums are
/// deliberately absent; their host layout would not define a portable ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    I64,
    U32,
    U64,
    /// Opaque fixed-size bytes; interpretation and element encoding are record-specific.
    Bytes(u16),
}

impl FieldKind {
    /// Storage extent in bytes, independent of the generator host.
    pub const fn size(self) -> u16 {
        match self {
            Self::U32 => 4,
            Self::I64 | Self::U64 => 8,
            Self::Bytes(size) => size,
        }
    }

    /// Natural wire alignment; byte arrays do not acquire their payload's alignment.
    pub const fn alignment(self) -> u8 {
        match self {
            Self::U32 => 4,
            Self::I64 | Self::U64 => 8,
            Self::Bytes(_) => 1,
        }
    }
}

/// Object-local method contract. Array order defines argument/result registers;
/// the receiver and signed return status each use separate registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Invocation {
    /// Common methods use 0..=5; object-specific methods start at 256.
    /// The same number may identify different methods on different object kinds.
    pub operation: u32,
    /// Target capability, carried separately from the business argument registers.
    pub receiver: Argument,
    pub name: &'static str,
    pub feature: FeatureGate,
    /// Business arguments in register order; the receiver is not repeated here.
    pub arguments: &'static [Argument],
    /// Auxiliary success results in register order, excluding signed status.
    pub results: &'static [ResultValue],
    pub blocking: BlockingClass,
    pub cancellation: CancellationClass,
    pub restart: RestartClass,
    pub completion: CompletionClass,
    pub audit: AuditClass,
    pub flags: FlagPolicy,
    /// Auxiliary results which remain defined for specific failure statuses.
    pub failure_results: &'static [FailureResults],
}

impl Invocation {
    /// Logical inputs including the separately carried target capability.
    pub fn inputs(&self) -> impl Iterator<Item = &Argument> {
        core::iter::once(&self.receiver).chain(self.arguments.iter())
    }
}

/// A borrowed receiver of an exact object kind. Consuming or polymorphic
/// receivers must supply their own declaration instead of using this helper.
pub(super) const fn invocation_receiver(kind: &'static str, required_rights: u64) -> Argument {
    Argument {
        name: "receiver",
        kind: ValueKind::Handle,
        handle: Some(HandleArgument {
            object: ObjectConstraint::Kind(kind),
            required_rights,
            disposition: HandleDisposition::Borrow,
        }),
        memory: None,
    }
}

/// Auxiliary results valid on a named failure, overriding the default OK-only rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureResults {
    /// Name of a declared non-OK status.
    pub status: &'static str,
    /// Names from the invocation's result list, not register numbers.
    pub results: &'static [&'static str],
}

/// Feature advertisement required by a method, distinct from kernel build cfg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureGate {
    Core,
    Named(&'static str),
}

/// One input register and its optional authority or pointed-to-memory contract.
/// Names are also references used by lengths, selectors and rights derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argument {
    pub name: &'static str,
    pub kind: ValueKind,
    /// Required for a handle value; does not describe handles embedded in memory.
    pub handle: Option<HandleArgument>,
    /// Checked user-copy range; special address-based methods define their access
    /// separately in semantic rules rather than acquiring generic copy authority.
    pub memory: Option<UserMemory>,
}

/// One auxiliary output register. A produced handle becomes usable only when
/// publication commits; non-OK exceptions must be listed in [`FailureResults`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultValue {
    pub name: &'static str,
    pub kind: ValueKind,
    pub handle: Option<ProducedHandle>,
}

/// Logical meaning of a register value, rather than the register's machine width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    U32,
    U64,
    I64,
    /// Process-local capability identifier, not a global KOID.
    Handle,
    /// Caller virtual address; [`UserMemory`] specifies whether/how it is accessed.
    UserAddress,
    ByteCount,
    /// Count of records or elements; multiply by the declared element size safely.
    ElementCount,
    /// Already shifted u64 capability-right mask.
    Rights,
}

/// Admission and ownership rules for one directly supplied capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleArgument {
    pub object: ObjectConstraint,
    /// Unconditional rights; selector-dependent operations can require more.
    pub required_rights: u64,
    pub disposition: HandleDisposition,
}

/// Object identity restriction independent of the handle's rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectConstraint {
    /// Any valid object kind; not permission to accept a missing or stale handle.
    Any,
    Kind(&'static str),
}

/// Ownership of an input handle at the operation's commit point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleDisposition {
    /// Validate and retain only for this call; do not remove the caller's handle.
    Borrow,
    /// Remove the caller's handle only when the operation commits successfully.
    ConsumeOnCommit,
    /// A scalar selector determines borrowing or consuming and extra rights.
    ByOperation {
        /// Name of the scalar argument selecting an entry in `operations`.
        argument: &'static str,
        operations: &'static [HandleOperation],
    },
}

/// One admitted selector value and the additional authority it requires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleOperation {
    pub name: &'static str,
    pub value: u32,
    pub disposition: OperationDisposition,
    /// Rights required in addition to the handle argument's common rights.
    pub additional_rights: u64,
}

/// Ownership effect after resolving a selector; unlike [`HandleDisposition`],
/// this cannot defer to a second selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationDisposition {
    Borrow,
    ConsumeOnCommit,
}

/// Kind and authority of a handle published into the caller's handle table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProducedHandle {
    pub object: ProducedObject,
    pub rights: ProducedRights,
}

/// Object kind of a newly published handle, either fixed or source-dependent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducedObject {
    SameAsArgument(&'static str),
    Kind(&'static str),
}

/// Authority of a newly published handle; no variant implicitly grants all rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducedRights {
    /// Requested-rights argument; the method must reject escalation over its source.
    RequestedSubsetOf(&'static str),
    /// Source-handle rights intersected with a fixed destination ceiling.
    InheritedSubset {
        authority_source: &'static str,
        allowed_rights: u64,
    },
    /// Grant exactly the requested mask within the ceiling; do not silently clamp it.
    ExactRequested {
        argument: &'static str,
        allowed_rights: u64,
        /// Optional source handle which must already carry every requested
        /// right before the produced handle can be published.
        authority_source: Option<&'static str>,
    },
    /// Fixed rights plus the allowed subset already held on the authority source.
    FixedAndInherited {
        fixed_rights: u64,
        authority_source: &'static str,
        inherited_rights: u64,
    },
    /// Precisely this mask, independent of caller-selected rights.
    Fixed(u64),
}

/// Checked user-memory access, including any handles embedded in wire records.
/// Direction is from the kernel's perspective; validation order is operation-local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserMemory {
    pub direction: MemoryDirection,
    pub length: MemoryLength,
    /// Layout name for typed memory, or None for unstructured bytes.
    pub record: Option<&'static str>,
    /// Capability effects inside the buffer; absent for plain data.
    pub handles: Option<IndirectHandles>,
    /// Relative order of checked user-memory ranges; unique within an invocation.
    pub validation_order: u8,
}

/// Bounds checked before touching user memory. Element counts are multiplied by
/// the explicit element size with overflow checks, not interpreted as bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLength {
    /// Exact fixed extent without a separate caller-supplied length register.
    FixedBytes {
        /// Accessed extent, not a caller-selected capacity.
        bytes: u32,
    },
    /// Byte count comes from a named argument and is bounded before access.
    Bytes {
        argument: &'static str,
        maximum_bytes: u32,
    },
    /// Element count comes from a named argument; `element_size` is measured in bytes.
    Elements {
        argument: &'static str,
        maximum_elements: u32,
        element_size: u16,
    },
    /// Record array with a caller-selected byte stride and bounded extent.
    StridedElements {
        argument: &'static str,
        maximum_elements: u32,
        stride_argument: &'static str,
        maximum_stride: u16,
    },
}

/// Authority carried inside a typed buffer, separate from the buffer's address.
/// Field names must resolve to fields of the record declared in [`UserMemory`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndirectHandles {
    /// Input records borrow handles for the duration of one syscall.
    BorrowRecords {
        handle_field: &'static str,
        required_rights: u64,
    },
    /// Input records describe conditional borrow-or-move operations which all
    /// commit together only when the syscall returns `OK`.
    ConsumeRecords {
        handle_field: &'static str,
        rights_field: &'static str,
        expected_kind_field: &'static str,
        operation_field: &'static str,
        common_rights: u64,
        operations: &'static [HandleOperation],
        commit: CapabilityCommit,
    },
    /// In/out records declare exact receive authority before the kernel
    /// publishes transferred handles into them.
    ProduceTransferred {
        handle_field: &'static str,
        rights_field: &'static str,
        expected_kind_field: &'static str,
        flags_field: &'static str,
        commit: CapabilityCommit,
    },
}

/// Commit rule for a multi-handle publication or transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityCommit {
    /// Capability ownership and live-handle publication commit only on `OK`.
    ///
    /// User copies may have partially modified output storage when the call
    /// reports `FAULT`; those bytes never constitute a published capability.
    AtomicOnOk,
}

/// Direction from the kernel's perspective, not the application's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryDirection {
    /// Copy input from userspace.
    Read,
    /// Publish output to userspace.
    Write,
    /// Validate input and publish output in the same range.
    ReadWrite,
}

/// Whether a method may suspend its caller; this is not a timing guarantee.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockingClass {
    Never,
    MayBlock,
}

/// Whether the method declares an explicit cancellation path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationClass {
    None,
    Explicit,
}

/// Kernel restart policy; no method currently promises transparent replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartClass {
    Never,
}

/// Successful control-flow outcome; failure still follows the method's contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionClass {
    Returns,
    NoReturn,
}

/// Logical auditing category, independent of capability admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditClass {
    Abi,
    Capability,
    Object,
    Task,
}

/// Treatment of a method's options/flags, not the rights mask.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagPolicy {
    /// No flag-handling policy is attached here.
    None,
    /// Only explicitly admitted bits/values are valid.
    Strict,
    /// The individual method defines which additional bits may be tolerated.
    Flexible,
}
