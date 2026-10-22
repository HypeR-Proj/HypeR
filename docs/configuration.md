<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Configuring a product

HypeR separates kernel capabilities, deployed userspace components and board
resources. A product can omit applications, background services and filesystem
workers without changing the kernel's startup contract or embedding a new
deployment policy in the Linux I/O VM.

| Input | Owns |
| --- | --- |
| `CONFIG_FILE` | Kernel architecture, address widths, stack sizes and numeric limits |
| `KERNEL_FEATURES` | Optional production kernel capabilities |
| `SYSTEM_CONFIG` | Executable selection, selectable boot profiles, formal storage successors and optional kernel features |
| `BOARD_CONFIG` | Physical devices, guest definitions, disk partitions and mount policy |
| Component and service declarations | Build targets, required child programs, startup and capability grants |

The ordinary build remains the full system with test programs excluded. Start
with [the default selection](../configs/default.json), or use
[the minimal selection](../configs/minimal.json) and
[its QEMU board](../boards/qemu-minimal.json):

```sh
make board-build ARCH=aarch64 \
  SYSTEM_CONFIG=configs/minimal.json BOARD_CONFIG=boards/qemu-minimal.json
make board-run ARCH=aarch64 \
  SYSTEM_CONFIG=configs/minimal.json BOARD_CONFIG=boards/qemu-minimal.json
```

`BOARD` remains the `qemu` platform; `BOARD_CONFIG` selects its concrete resource
and partition layout. A configuration file name does not introduce another
firmware or I/O VM platform.

The minimal product keeps the bootstrap handoff, I/O and EROFS services,
component management, journal and an interactive shell with a few diagnostic
tools. It omits FAT workers, the managed guest fleet and most ordinary tools.
Its board has only the independent system partition and no persistent data
mount. This is a runnable product selection, not a kernel self-test image.
Other system-only selections may retain VM manager: their empty initial fleet
is packaged at `/etc/hyper/vms.json` rather than referring to an absent `/data`.

## Userspace selection

`SYSTEM_CONFIG` is a versioned JSON document. `boot` and `system` select
executable components using their staged names from `make app-manifest`:

```json
{
  "version": 1,
  "boot": {
    "base": "default",
    "exclude": ["fs-fat"]
  },
  "system": {
    "base": "default",
    "exclude": ["fs-fat-system", "editor", "less"]
  }
}
```

`base: "default"` starts from the ordinary selection for that environment.
`base: "none"` starts from no executables and `include` is an explicit allowlist.
Omitted lists are empty. A name may not occur in both `include` and `exclude`.
Unknown names, wrong-environment names, duplicate keys and unsupported fields
are errors. Bootstrap and formal names are independent even when they install
to the same relative path: `fs-fat` and `fs-fat-system` both become `svc/fs-fat`
inside their own filesystem. Some staged names contain a directory, such as
`svc/journal`; use the registry name exactly.

Selection controls Cargo executable targets, installation and final image
membership together. Shared-library providers are built from the selected
Cargo dependency graph; the packer follows each executable's actual `DT_NEEDED`
closure. A previous build's unused executables or libraries do not become part
of a trimmed image. Source-owned bootstrap executables remain statically linked
and cannot acquire formal userspace libraries through product configuration.
The existing debug-symbol policy is unchanged.
Cargo fetch and lockfiles remain workspace-wide: a trimmed product avoids
compiling and installing omitted programs, but does not maintain a separate
dependency lockfile or restrict downloads to its selected executable graph.

Removing an optional formal service removes its startup declaration as well.
Programs that launch other programs declare their runtime requirements: for
example, session-manager needs terminal-service and sh. A selection that keeps
a launcher while omitting a required executable fails with a dependency error;
the build does not silently add an excluded program back. The bootstrap
supervisor and formal ownership-adoption path remain necessary for this product
model. Their required executables cannot be removed while still selecting that
startup path.

Filesystem support remains discovered at runtime. The filesystem manager
recognizes the volume magic and locates the corresponding worker. Omitting
`fs-fat` does not make FAT mount syntax invalid at build time: requesting a FAT
mount without any reachable FAT worker produces an explicit runtime error.
Change the board's fstab when an omitted filesystem is no longer intended to be
mounted. The generated boot profile declares the worker for the board's known
system filesystem: omitting that worker now fails archive admission before
boot. The builder reports the missing dependency instead of silently restoring
an excluded program. Custom profiles declare their own early worker dependencies.

Ordinary `system` profiles exclude test binaries. An explicit product `include`
may opt into a registered test program regardless of profile membership;
runtime acceptance uses this to add its command reporter to the minimal product.
Development fixtures may also select their test profile explicitly. Neither
mechanism turns on kernel fault injection or self-tests.

## Selectable boot profiles

Products may define up to 16 `boot-profiles`. The fixed `default` entry is
mandatory. Each is installed as `/etc/hyper/boot-profiles/NAME.json` in the
initial archive. Without this section, composition generates `default` from
the selected bootstrap service graph and board resources, preserving ordinary
I/O VM boot. Profile names contain 1–63 ASCII letters, digits, `_` or `-`.

```json
{
  "version": 1,
  "boot-profiles": {
    "default": {
      "version": 1,
      "services": "/etc/hyper/services.json",
      "io": {
        "kind": "io-vm",
        "service": "io-runtime",
        "successor": "/io-runtime"
      },
      "requires": [
        "/etc/fstab",
        "/etc/hyper/filesystem.json",
        "/etc/hyper/storage.json",
        "/etc/hyper/io-clients.conf",
        "/svc/fs-erofs"
      ],
      "handoff": {
        "system-root": "/system",
        "put-old": "/boot",
        "system-init": "/svc/component-manager"
      }
    }
  }
}
```

`services` names an archive-local service manifest. The ordinary manifest at
`/etc/hyper/services.json` is generated from the configured service graph;
alternate manifests must be explicitly packaged as boot files. Profile paths
are canonical absolute paths; `requires` lists up to 128 additional files the
provider or its children need before handoff. The builder checks these against
the archive itself, including archive-local symlinks. A file present only in the
formal system or build directory cannot satisfy a bootstrap dependency.

Every shipped profile is checked, including unselected profiles. Their service
graphs use the production stage1 parser and capability policy. Each service
must have a packed, executable static ELF matching the bootstrap architecture.
For `io-vm`, the packed board configuration and the referenced I/O VM FIT must
also exist. Dynamic runtime choices, such as a format discovered only by
reading disk magic, still need explicit `requires` declarations when they are
critical to reaching the formal root; this is dependency admission, not a
simulation of arbitrary service code.

The kernel forwards the boot command line without interpreting profile policy.
Stage1 selects `default` unless it sees `hyper.boot-profile=NAME`. Unknown,
malformed or repeated selectors produce a bootstrap error instead of selecting
another profile. QEMU accepts this token in the kernel `-append` command line;
Pi firmware accepts it in `cmdline.txt`.

The I/O source chooses the bootstrap provider independently of root handoff:

| `io.kind` | Meaning |
| --- | --- |
| `io-vm` | Start the named provider with the explicit VM creation capability; the shipped implementation is `io-runtime` |
| `service` | Start a named userspace provider with no VM creation capability; its executable may implement a different storage source |
| `none` | No selected storage provider; a root needed for handoff must already be independently prepared |

The generic service option defines the integration boundary; this patch does
not add a Native physical block driver. A provider receives only the declared
capabilities, supplies readiness when it supplies filesystem volumes, and owns
its private migration protocol. A recovery profile may omit `handoff`; a
provider's `successor` is required only when handoff is configured.

Formal storage candidates are configured separately, for example:

```json
{
  "version": 1,
  "system": {
    "storage-providers": [
      {"component": "/io-runtime", "image": "/svc/io-runtime"},
      {"component": "/disk-driver", "image": "/svc/disk-driver"}
    ]
  }
}
```

The example's custom driver must be implemented and explicitly deployed before
that candidate is usable. Composition declares these as prepared resident
storage components; only the selected profile's `successor` is adopted. An
unlisted successor is rejected. If the list is absent, composition declares the
shipped `/io-runtime` successor only when a handoff profile uses `io-vm`.
The common handoff transports an opaque migration endpoint; it does not parse
the I/O VM's private description. See [the handoff contract](bootstrap-handoff.md).

Profiles still have to satisfy the chosen formal services. For example, the
current VM manager's I/O broker grant requires a bootstrap provider offering
that broker; a generic driver does not acquire one automatically. Such known
missing grants fail composition. Use a compatible formal service selection.

The board's `io-vm` section is optional for products whose sources do not need
an I/O VM; such products must define explicit boot profiles for their alternate
source rather than relying on the generated I/O VM default. All included
profiles contribute to the appliance requirement: if
any profile needs one, the board section and image are required, even when it
is not the default. If none does, board composition skips appliance fetching
and packaging, including stale appliance entries left by an earlier build.
The current guest disk/network declarations use the I/O VM backend and still
require its board section. Removing that section alone does not implement a
replacement guest backend.

## Kernel features

`KERNEL_FEATURES` is an exact comma-separated production feature set. An empty
value builds without these optional capabilities. A product can carry the same
selection in its JSON document:

```json
{
  "version": 1,
  "kernel": {
    "features": ["runtime-symbols"]
  }
}
```

| Feature | Behavior when disabled |
| --- | --- |
| `rtc` | No hardware wall-clock initialization; monotonic time remains available |
| `runtime-symbols` | No embedded runtime symbol table; crash reports retain raw addresses |
| `crash-console` | No interactive kernel crash command loop; fatal reporting remains |
| `pmu-profiling` | No AArch64 PMUv3 IRQ sampler or sample buffers; PMU queries report unsupported |
| `heap-profiling` | No per-size-class heap occupancy and traffic counters; general memory accounting remains |

The default feature set preserves the existing production behavior.
`pmu-profiling` is opt-in; see [PMU sampling](profiling.md).
`heap-profiling` is opt-in; see [memory diagnostics](memory-diagnostics.md).
Memory ownership, capability checks, device isolation, IPC and the scheduler are
not optional security switches. The architecture and limit configuration stays
in the kernel's Kconfig file; a capability has one feature owner instead of
competing Cargo and Kconfig toggles.

Older local kernel configurations may contain `CONFIG_CRASH_CONSOLE` or
`CONFIG_HEAP_PROFILING`. Run `make olddefconfig ARCH=aarch64` to remove obsolete
settings with an explicit warning while retaining recognized address, stack and
resource limits. Select the corresponding Cargo features separately if needed;
the removed values do not silently add features. Do not reset an otherwise
customized configuration merely to change optional functionality.

## Test builds

All kernel test selectors require two explicit gates: `kernel-test` and the
individual selector. Neither gate is enabled by production defaults, and a
selector never enables the master gate as a dependency. Selecting a test
without `kernel-test` is a build error. Test code also uses both conditions at
its compilation boundary.

| Selector | Purpose |
| --- | --- |
| `kernel-self-test` | Standalone kernel mechanism tests instead of Native startup |
| `kernel-smmuv3-test` | Standalone SMMUv3 isolation tests, including the self-test harness |
| `kernel-vgic-tc-test` | Force the GICv3 common-register trap path |
| `kernel-stack-audit` | Stack watermark instrumentation used by runtime acceptance |
| `io-dma-revoke-test` | Test-only revocation of a live production DMA buffer |

The supported test targets supply both gates. For an explicit test build:

```sh
make image ARCH=aarch64 \
  CARGO_FEATURES='--features kernel-test,kernel-self-test'
```

`kernel-test` alone does not replace production startup with self-tests. Ordinary
host `#[cfg(test)]` unit tests remain a separate compiler test mode; they do not
enter a delivered kernel image. Product JSON accepts production feature names,
not test selectors. Avoid Cargo's `--all-features` for a production image.

## Incremental builds and persistent disks

Custom product selections use configuration-specific kernel, application and
board output directories. Identity follows normalized configuration contents, so
editing a file in place selects new outputs; reordering a set does not create a
different build. Default invocations keep the existing output locations.

Normal board builds continue to update generated system contents while
preserving persistent partition bytes. Selecting a smaller image does not grant
permission to repartition or erase an existing data volume. To change storage
geometry, use a separate output directory or the explicitly destructive rebuild
target. See [board storage](board-storage.md) and
[component builds](../mk/README.md) for output and packaging details.
