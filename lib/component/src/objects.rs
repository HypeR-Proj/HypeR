// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Typed Native objects in a configuration-selected grant inventory. Type erasure
//! stays inside this enum; all kernel transfers use the ordinary typed SDK APIs.

use hyper_os::{Error, Result, capability_channel::*, handle::*, task::ProcessBuilder};

macro_rules! objects {
    ($($variant:ident($ty:ty)),+ $(,)?) => {
        pub enum Object { $($variant(Option<OwnedHandle<$ty>>)),+ }
        impl Object {
            pub fn receive_slot(kind: u32, rights: Rights) -> Result<CapabilityReceiveSlot> {
                $(if kind == <$ty>::KIND.as_raw() { return Ok(CapabilityReceiveSlot::new::<$ty>(rights)); })+
                Err(Error::InvalidResponse)
            }
            pub fn take(kind: u32, slot: &mut CapabilityReceiveSlot) -> Result<Self> {
                $(if kind == <$ty>::KIND.as_raw() { return Ok(Self::$variant(Some(slot.take::<$ty>()?.ok_or(Error::MissingHandle)?))); })+
                Err(Error::InvalidResponse)
            }
            pub fn take_owned(&mut self) -> Result<Self> {
                match self { $(Self::$variant(h) => Ok(Self::$variant(Some(h.take().ok_or(Error::MissingHandle)?)))),+ }
            }
            pub fn info(&self) -> Result<HandleInfo> {
                match self { $(Self::$variant(h) => h.as_ref().ok_or(Error::MissingHandle)?.info()),+ }
            }
            pub fn duplicate(&self, rights: Rights) -> Result<Self> {
                match self { $(Self::$variant(h) => Ok(Self::$variant(Some(h.as_ref().ok_or(Error::MissingHandle)?.duplicate(rights)?)))),+ }
            }
            pub fn move_offer(&mut self, rights: Rights) -> Result<CapabilityDisposition<'_>> {
                match self { $(Self::$variant(h) => CapabilityDisposition::move_handle(h, RightsOffer::Exact(rights))),+ }
            }
            pub fn install(&mut self, builder: &ProcessBuilder, purpose: u32, rights: Rights, transfer: hyper_component_config::Transfer) -> Result<()> {
                match self { $(Self::$variant(h) => match transfer {
                    hyper_component_config::Transfer::Duplicate => builder.add_handle_duplicate(h.as_ref().ok_or(Error::MissingHandle)?.as_handle_ref(), purpose, RightsOffer::Exact(rights)),
                    hyper_component_config::Transfer::Move => {
                        let value = h.take().ok_or(Error::MissingHandle)?;
                        builder.add_handle_move(value, purpose, RightsOffer::Exact(rights)).map_err(|failure| {
                            let (error, value) = failure.into_parts(); *h = Some(value); error
                        })
                    }
                }),+ }
            }
        }
    };
}

objects! {
    Directory(DirectoryObject), File(FileObject), Vmo(VmoObject),
    Channel(ByteChannelObject), CapabilityChannel(CapabilityChannelObject),
    Factory(TaskFactoryObject), Group(TaskGroupObject), Domain(ResourceDomainObject),
    Process(ProcessObject), Builder(ProcessBuilderObject), Console(ConsoleObject),
    TaskInspector(TaskInspectorObject), ObjectInspector(ObjectInspectorObject),
    MemoryInspector(MemoryInspectorObject), CpuInspector(CpuInspectorObject),
    DeviceAuthority(DeviceAssignmentAuthorityObject), VmAuthority(VirtualMachineCreationAuthorityObject),
    RootTransition(RootTransitionObject),
}
