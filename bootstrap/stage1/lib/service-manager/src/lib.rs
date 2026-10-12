// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bootstrap service policy and supervision; independent of system init.
//!
//! Parsing and planning deliberately depend on no operating-system binding.
//! The runtime adapter must obtain real root-`Directory` and process-construction
//! capabilities before it can apply a validated launch plan.
//! Core supervision transfers in one capability rendezvous. The successor owns
//! namespace switching and system services after that commit.

pub mod bootstrap_policy;
pub mod diagnostics;
pub mod manifest;
pub mod supervision;

pub mod runtime;
