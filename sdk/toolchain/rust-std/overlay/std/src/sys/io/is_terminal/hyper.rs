// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::io::{Stdin, StdinLock, Stdout, StdoutLock, Stderr, StderrLock};
pub trait TerminalHandle { fn stream(&self) -> Option<u32>; }
impl TerminalHandle for crate::fs::File { fn stream(&self) -> Option<u32> { None } }
macro_rules! streams {
    ($($ty:ty => $stream:expr),*) => {$(
        impl TerminalHandle for $ty { fn stream(&self) -> Option<u32> { Some($stream) } }
    )*};
}
streams!(Stdin => 0, StdinLock<'_> => 0, Stdout => 1, StdoutLock<'_> => 1, Stderr => 2, StderrLock<'_> => 2);
pub fn is_terminal(handle: &impl TerminalHandle) -> bool {
    handle.stream().is_some_and(|stream| unsafe { crate::sys::pal::ffi::__hyper_std_is_terminal(stream) != 0 })
}
