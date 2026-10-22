<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# `ldd`: shared-library dependency inspection

[ldd](../../../app/ldd/) reads an executable or shared object's ELF metadata and reports its
interpreter and complete `DT_NEEDED` dependency graph. It does not execute the
input, load it into executable memory, or call constructors. The default
listing includes each dependency name once; paths resolve symbolic links, so
the `/lib` alias is displayed as `/lib64/<arch>-hyper-hyper/`. The
ELF's declared interpreter is shown separately from the loader file being checked.

```text
ldd /bin/vmm
ldd --tree /svc/vm-runtime
ldd --direct /bin/ps
ldd -v /bin/vmm /bin/handle
ldd --library-dir /data/candidate-libraries /bin/vmm
ldd --loader /data/candidate-loader --library-dir /data/candidate-libraries /bin/vmm
```

`--tree` shows which object requires each library, marking cycles and already
shown subtrees. `--direct` checks only the input's immediate dependencies and
loader. `-v` includes architecture, ELF OS ABI/version and SONAME. Static
executables are identified explicitly, and `--loader FILE` also checks their
selected loader. Missing libraries or loaders print `not found`;
unreadable, malformed or incompatible files have specific diagnostics. Multiple
operands are processed independently, with a failing exit status if any inspected
dependency cannot be resolved or validated.

The lookup directory defaults to `/lib/<arch>-hyper-hyper/` for the
input ELF architecture. Every `PT_INTERP` path is inspected literally; the canonical Native interpreter
is `/lib64/ld-hyper-<arch>.so`. `--library-dir`
inspects an alternate library set, and `--loader FILE` checks an explicitly
selected executable loader. These options are independent and do not change
the target program's runtime launch policy. The ELF declaration remains
visible even when the inspected loader has a different filesystem path.
No environment search path, RPATH or RUNPATH is applied. The command needs only
ordinary filesystem read authority, not inspector privileges. A process supplied
a different library Directory
capability can see different files. Inspection is not atomic across file changes.

Parsing bounds metadata reads and dependency traversal to the Native loader's
object/name limits. Debug sections and entire library contents are not read.
The tool does not verify symbols, relocations, future `dlopen` calls or process
capability grants; a successful listing is not proof that the image can run.
No runtime load addresses are invented for files that have not been executed.
