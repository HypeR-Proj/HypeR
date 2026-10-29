<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Experimental Native C++ runtime

This component builds the libc++ subset needed by the Fuchsia F2FS adapter.
It consumes installed `hyper/thread.h`, `hyper/syscall.h` and `hyper/stream.h`;
it contains no syscall encodings or kernel source dependencies. Its static archive
and private headers are component outputs, not a promised general-purpose SDK
C++ distribution.

The runtime supports C++20 containers, atomics, mutexes, condition variables,
shared mutexes, clocks and threads in the private `std::__hyper_cxx` namespace.
Threads and TLS keys use the Native runtime; compiler-native ELF TLS is not used.
Exceptions, RTTI, static destruction, localization, filesystem and timezone
support are disabled. Fatal invariant and allocation failures terminate the
process with a diagnostic. The owner must explicitly join all threads and destroy
its mounts; process exit is not a filesystem sync operation.

`build.py` builds a pinned libc++ 21.1.8 source subset. musl 1.2.5 supplies private
C declarations only; no musl object or foreign syscall implementation is linked.
These declarations do not extend the Native SDK with a POSIX ABI. `os.cc` adapts
clock, sleep, yield and diagnostics to Native facilities. `runtime.cc` supplies
guards, thread startup and the small remaining C/C++ runtime surface.

Build with an installed AArch64 or RISC-V SDK:

```sh
python3 stdlib/cxx/build.py --sdk target/sdk/aarch64 \
  --output target/cxx/aarch64 --fetch
```

Ordinary builds use verified cached source archives and never fetch implicitly.
The F2FS build invokes this component with its own output directory. Source pins
live in `sources.json`; complete dependency licenses are in
[`lib/fs-f2fs/licenses`](../../lib/fs-f2fs/licenses/).
