<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Boot, system and data deployment

The default build keeps the storage startup chain in RAM at `/boot`, loads
ordinary Native programs from a dedicated read-only EROFS volume at `/system`,
and mounts the writable persistent FAT volume at `/data`. Plain `make`,
`make rebuild` and `make run` use this split layout without an extra option.
`BOARD_SYSTEM_LAYOUT=ramfs` changes only the system payload storage: `/system`
is included in the boot archive, with the same paths, loader selection and
supervision handoff.

The kernel mounts the supplied RAM archive directly as `/` and starts `/init`
through `/lib/userspace-loader-hyper-<arch>`. Those are the only bootstrap path
conventions in the kernel. `/boot`, `/system`, numbered bootstrap stages and
the service handoff are userspace deployment policy. After the handoff, `/boot`
is the original RAM filesystem mounted below the new root, not the Pi firmware's FAT boot partition.
Firmware files remain at the locations required by the board boot profile.

Boot terminology distinguishes **stage-1** bootstrap/recovery from the **system
environment**, whose supervisor is `init-system`. The system environment has no
stage number. Additional numbered bootstrap stages are reserved for possible
intermediate initialization, such as verified-storage setup; they are not
implemented here. The current handoff goes directly from stage-1 to system init.

## Files and dependencies

The RAM-resident program sources are grouped under
[`bootstrap/stage1`](../bootstrap/stage1/README.md). Ordinary programs and the
system init remain under `app/`; reusable implementations remain in `lib/`.
Source placement does not change the runtime paths below.

```text
/boot/init                         static stage-1 init
/boot/svc/fallback-sh              static recovery shell
/boot/bin/mount, umount             static filesystem administration tools
/boot/svc/                         static console, I/O and filesystem services
/boot/etc/hyper/services.json       core service graph
/boot/etc/hyper/init.json           successor executable path
/boot/etc/hyper/filesystem.json     filesystem-worker search directories
/boot/etc/hyper/storage.json        GPT partition selectors and access policy
/boot/etc/fstab                    bootstrap mounts (initially /etc/fstab)
/boot/lib64/                       static bootstrap userspace loader
/boot/vm/io.itb                    pinned I/O VM image
/system/bin/                       formal sh and ordinary Native commands
/system/svc/                       init-system, vm-manager, vm-runtime
/system/etc/hyper/services.json    system service graph
/system/etc/fstab                  mounts requested by formal init
/system/lib64/ld-hyper-<arch>.so    formal executable loader and dynamic linker
/system/lib64/<arch>-hyper-hyper/  shared libraries for system programs
/data/vms.json                    writable VM fleet configuration
/data/vm/                         business guest images
```

The archive initially contains `/init`, `/bin`, `/svc`, `/etc`, `/vm` and
`/lib64` directly, plus `/lib -> lib64`. System init prepares a separate ramfs
root and atomically switches the shared namespace to it. The original root
becomes `/boot`; the existing `/system` mount moves with it, without restarting
its worker. Formal init then requests the data mounts in `/system/etc/fstab`.
Any additional mounts explicitly selected for bootstrap also survive the switch. In an all-RAM
deployment, the switch promotes the RAM subtree into a mount and removes its
old directory entry in the same transaction, without copying its files.

The formal root contains only `/boot`, `/system`, `/data` and `/tmp` by default.
There are no root-level `/bin`, `/svc`, `/etc`, `/vm`, `/lib` or `/lib64` aliases.
Additional configured volume mountpoints remain explicit. Empty covered mountpoint
and staging directories are removed from `/boot` after the switch; cleanup never
recursively removes contents. The all-RAM system tree has only the `/system`
entry, not a second `/boot/system` entry.

Core launchers receive a fixed `process.environment-directory` capability for
image and configuration lookup. It remains attached to the boot filesystem when
that filesystem moves from `/` to `/boot`. Their separate `process.root-directory`
capability follows the live namespace and is used for ordinary paths and mount
targets. System launchers use a fixed `/system` environment. The shell opens
`mount` and `umount` in its boot environment's `bin`; other bare commands, including `sh`,
resolve under `/system/bin`. Explicit paths are resolved as written.

Only explicit namespace-root directory capabilities follow a root switch,
including duplicates and scopes derived from that anchor. An opened file or
fixed subdirectory capability keeps its original identity and confinement.
A scoped cwd in the retained old root remains usable below `/boot`. A contextual
cwd inside a promoted RAM subtree that still names the old mount is detached:
relative lookup fails instead of fabricating an old path. A directory confined
to that subtree and existing File capabilities remain usable. New opens use the
new mount identity while preserving filesystem nodes and cached file contents.
Root and mount edges publish as one immutable snapshot, so lookups cannot observe
a new root with the old mount table. The generic kernel operations accept paths
from userspace and know nothing about `/boot`, `/system` or bootstrap stages.

Core services explicitly use the bootstrap loader, which accepts static ELF
only. System programs, including static system executables, use the separately
delegated `/system/lib64/ld-hyper-<arch>.so` executable. It maps the program and
performs dynamic linking when needed. A dynamic ELF's `PT_INTERP` still identifies
`/lib64/ld-hyper-<arch>.so`; it does not force the launcher to locate that file
through the process root. Each system process receives the library Directory
`/system/lib64/<arch>-hyper-hyper`. The loader and libraries are on disk in split
deployment and in RAM in ramfs deployment. Bootstrap and recovery do not need
the formal loader.

The default RAM selection includes init, console-raw, fallback-sh, io-runtime,
fs-backend, fs-erofs, fs-fat, mount and umount. It is selected through
`APP_BOOT_COMPONENTS`, using component staged names, and built separately with
shared-library delivery disabled. Rust boot services use statically linked std; fallback-sh omits clap and advanced shell features. Storage startup and recovery commands do not require any
library on `/system`. The static FAT worker lives in `/boot/svc/fs-fat` by default
and is loaded on demand. Its placement does not change the mount phase: formal
init still requests `/data` through `/system/etc/fstab`. Removing `fs-fat` from
`APP_BOOT_COMPONENTS` places it on `/system` instead; the configured worker search
supports either location. Ordinary tools are not duplicated in `/boot`.

Original ELF build products retain debug information. Packaged copies use the
existing debug-section stripping policy. System programs and their transitive
DSO dependencies are checked as one build, including architecture and symbol
closure; changing file placement does not relax the dynamic-link contract.
For an all-RAM archive, validation uses the explicit `system` packaging scope
for both the delegated loader and `DT_NEEDED` libraries. This build policy does
not create a VFS namespace.

Dependency inspection follows these system locations by default:

```sh
ldd /system/bin/vmm
```

The tool still reports the ELF's declared `PT_INTERP`. For the canonical
`/lib64/ld-hyper-<arch>.so` name it checks the loader under `/system/lib64`;
other interpreter paths are inspected literally. `--loader` and `--library-dir`
can select another deployment explicitly. These inspection choices do not
change the executable's runtime launch policy.

## Supervision handoff

Stage-1 init validates and starts core services, including the console owner,
then waits for the filesystem manager's bootstrap mount result from the RAM
filesystem's `/etc/fstab` (only `/system` by default). No interactive shell
is launched while this startup decision is pending. The raw Console transport
contains no line discipline or formal terminal dependency.
The I/O VM starts through io-runtime independently of the business VM manager.
Its VM creation authority belongs to core services; no business fleet domain or
provisioning channel is needed to start or retain Native storage. System init allocates the business fleet scope only when launching
vm-manager. It closes unused business endpoints if the manager is omitted or
fails to launch. The default manager is noncritical, so its failure does not
terminate the I/O VM or filesystem services.
The kernel does not parse fstab or perform this service policy.

After that initial mount attempt, init checks the system-init executable and
its library directory. If they are available, it starts `/system/svc/init-system`
in a separate candidate task group. The candidate reads both service graphs,
validates their combined dependencies and its typed capability inventory, then
registers a receive operation. One capability-channel rendezvous transfers the existing
core Process supervisors and the management inventory. That successful transfer
is the supervisor-ownership commit point: stage-1 init exits and system init
owns the already-running core services. System init then prepares and commits
the configured root switch, requests automatic mounts from `/system/etc/fstab`,
then launches formal services and configures the VM fleet. A later mount failure
is reported by formal init; it does not undo handoff or prevent unrelated Native
services from starting. It does not restart
the console, I/O VM or filesystem workers during these transitions. Formal init launches `/system/svc/session-manager`, which owns the formal
terminal and `/system/bin/sh` session. Stage-1 init alone launches and supervises
`svc/fallback-sh` after a pre-handoff failure. That fallback uses byte streams
without terminal-service. There is no shared shell-selection decision channel:
a failure after committed handoff belongs to formal init and cannot resurrect
bootstrap recovery. The raw Console service and its log channels survive the
handoff; background logs remain mirrored alongside interactive output.

The boot `/etc/hyper/init.json` selects only the successor executable. Formal
init reads `/system/etc/hyper/init.json`, which selects its service manifest and
supplies the root-switch plan: the new root staging path,
the old-root attachment path, directories and mount moves. Custom policies may
explicitly request links; the default policy creates none.
The staging path and mount sources refer to the old namespace; the old-root
attachment and mount targets refer to the prepared new root, even when they
start with `/`. The default staging directory is `/.newroot`, which must not
already exist. Preparing ramfs and changing the VFS root
require `MANAGE_NAMESPACE | WRITE | EXECUTE` on a namespace-root Directory,
held by init and transferred to its successor. Ordinary shell and filesystem-worker
directory grants do not include `MANAGE_NAMESPACE`. A preparation or switch
failure leaves the old root active; the successor retains core supervision and starts the formal shell
instead of dropping their process owners or launching formal services. The
supervisor transfer and namespace switch are separate, explicit commit points.
The old ramdisk remains mounted and resident for core services and recovery;
switching roots does not itself reclaim its backing memory.

Nested disk mounts move with their mounted ancestor. If an optional ancestor
is absent, an existing child mount is moved separately. RAM subtree promotion
rejects mounted descendants: a configured child below an all-RAM subtree reports an
error while preserving core supervision and the formal console.

An unavailable `/system`, a missing system-init executable, a rejected candidate graph,
or a failed pre-commit handshake leaves core supervision and the recovery shell
in stage-1. An unrelated data-volume failure is handled by formal init and does
not prevent handoff when `/system` is already available. In particular, ramfs deployment can adopt core
supervision and run system tools without any disk; VM fleet provisioning still
requires its configured file, normally `/data/vms.json`.

After a failed handoff attempt, recovery mount commands can repair the namespace,
but they do not repeat the handoff or launch system services. There is no
`initctl` retry command; startup into the system environment must be attempted
on the next boot. After commit, critical-service failures follow the configured supervisor policy; the handoff
does not make an arbitrary critical system service failure recoverable.
The formal shell retains its existing features under `/system/bin/sh`. Recovery
uses `/boot/svc/fallback-sh`, without clap, history, pipelines or redirection.
No `svc` directory is searched for commands. Stage-1 init directly supervises
fallback before handoff; formal init launches `session-manager` afterward.
The manager owns the formal terminal and shell, with bounded retries after
launch failure or rapid exit. It never switches back to the bootstrap shell.

## Runtime filesystem policy

Each environment owns its fstab. The initial RAM root's `/etc/fstab`, retained as
`/boot/etc/fstab` after handoff, contains only bootstrap mounts:

```fstab
PARTLABEL=hyper-system /system auto ro 0 0
```

The system payload's `/system/etc/fstab` contains later mounts:

```fstab
PARTLABEL=hyper-data /data auto rw 0 0
```

Stage-1 never reads the formal table. Formal init requests it after adopting core
supervision and switching roots. An empty formal table needs no filesystem service;
in an all-RAM deployment the bootstrap table is empty. There are no numeric stage
flags, and the kernel never reads either table.

Sources select authorized GPT partitions with `PARTUUID=` or `PARTLABEL=`.
They do not name Linux device paths. HypeR reads the GPT and grants the selected
ranges before the filesystem manager sees them. The manager
identifies the media by magic and loads one separate format-worker process per
mount. `auto` selects the recognized format; explicit `fat`, `vfat`, or `erofs`
requires a matching format. EROFS mounts require `ro`. Supported options are
`defaults`, `ro`, `rw`, `auto`, `noauto` and `nofail`. The final two fields must
be `0 0`; fsck and dump scheduling are not implemented. Entries run in file
order within the selected table. A required bootstrap failure prevents bootstrap
readiness. A required formal failure is reported after handoff; the formal shell
and unrelated system services remain available. `nofail` makes an entry
optional, and `noauto` leaves it for an explicit mount command.

`/boot/etc/hyper/filesystem.json` (initially `/etc/hyper/filesystem.json`)
contains only runtime worker search policy:

```json
{
  "worker_directories": [{"environment": "svc"}, {"namespace": "/system/svc"}]
}
```

`environment` paths are relative to the filesystem manager's fixed boot directory
and use the static bootstrap loader. `namespace` paths are absolute in the live
namespace and use the formal loader and libraries from `/system`. Mount targets
always use the live namespace. Startup reads the bootstrap fstab and worker policy
through the fixed boot directory. Administrative requests carry an explicit fstab
path in the live namespace; the default is `/system/etc/fstab`. Worker policy
continues to come from the boot environment. Neither lookup requires root aliases.

A format needed to mount `/system` must have its worker in an already available
filesystem. Identifying EROFS does not require its worker to be built in: if
`fs-erofs` is omitted, the mount reports the missing worker at runtime. The
builder does not enforce a provider dependency graph. Unknown media, missing
workers and unsupported format features are distinct errors.

```text
mount                               list managed mounts
mount -a                            apply /system/etc/fstab
mount -T /boot/etc/fstab -a          retry bootstrap entries after root switch
mount -T /etc/fstab -a               retry bootstrap entries before root switch
mount /data                         mount one fstab target
mount -t auto -o ro PARTLABEL=hyper-system /system   select a GPT partition
umount /data                        sync and detach an idle mount
```

Unmount refuses open files, directory capabilities, active lookups and nested
mounts. Protected file mappings also keep their backing mount busy until
they retire. There is no forced or lazy unmount. Successful unmount drains
filesystem RPC, retires the old mount/cache generation, stops the worker, and
returns its exclusive block capability to the manager for a later mount.
A dead worker or unfinished block transaction is not silently reactivated.
See [filesystem services](filesystem-services.md) for the failure contract.

## Build and customize

```sh
make
make run
make rpi5-sd
```

The builder uses uncompressed core-format EROFS, 4096-byte blocks, no xattrs,
a fixed timestamp and cleared UUID. `mkfs.erofs` is a host build tool; the
filesystem implementation remains a Native process. `EROFS_MKFS` selects an
existing executable and does not install software globally.

QEMU outputs default to `target/board/qemu/`; Pi 5 outputs default to
`target/board/rpi5-native/`. `bootstrap.cpio` contains RAM files, `system-root/`
is checked staging, and `disk.img` contains the independent Native/guest
volumes. A build-side `disk.img.system.json` records system payload hashes.
It detects a stale disk paired with a rebuilt system payload; it is not runtime
integrity verification. Existing disks are preserved. A changed system payload
requires an explicit disk replacement or a fresh output directory before it can
run. `make rebuild` (or `make board-rebuild`) replaces the entire disk, including
data and guest volumes. Pi image replacement requires the existing `BOARD_IMAGE_REPLACE=--replace` option.

For the same namespace with system programs resident in RAM, select
`BOARD_SYSTEM_LAYOUT=ramfs` on both build and run:

```sh
make BOARD_SYSTEM_LAYOUT=ramfs
make run BOARD_SYSTEM_LAYOUT=ramfs
```

This uses `target/board/qemu-ramfs/`; Pi 5 uses
`target/board/rpi5-native-ramfs/`. Its bootstrap fstab is empty; the formal fstab still mounts `/data`.
Partition capacities and content belong to `disk.partitions`. Mount policy is
separate, for example:

```json
"native": {
  "layout": "split",
  "volumes": [
    {"source": "PARTLABEL=hyper-data", "read-only": false},
    {"source": "PARTLABEL=hyper-system", "read-only": true}
  ],
  "boot-fstab": "PARTLABEL=hyper-system /system auto ro 0 0\n",
  "fstab": "PARTLABEL=hyper-data /data auto rw 0 0\n"
}
```

Every Native partition consumes one data client, sharing eight slots with guests.
Client zero is the owner-only physical block control channel. Labels are not
reserved and may be changed together with their selectors; PARTUUID selects a
specific partition even when several disks contain the same label. HypeR's
`etc/hyper/storage.json` owns these selections. The Linux backend receives only
validated ranges and never resolves a logical volume name.
See [board storage](board-storage.md) for the explicit partition schema and inputs.

Functional checks live in `tests/qemu/verify-boot-system.py`; its `--recovery`
mode omits storage and checks shell survival. Combined with `--ramfs`, it also
requires successful handoff and working RAM-resident system commands. Parser
bounds and EROFS fixture checks run with
`cargo test --manifest-path bootstrap/stage1/Cargo.toml -p bootstrap-erofs`.
`tests/qemu/verify-vm-manager-isolation.py` takes a built split deployment and
creates private test disks. It exercises Native reads, writes and remounts with
vm-manager omitted, missing, exiting before configuration, closing or corrupting
its reply, exiting after acceptance, and running without a fleet configuration.
Its C stand-ins are test-only and are never packaged in normal deployments.
These checks establish behavior, not disk throughput or physical durability.
