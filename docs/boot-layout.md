<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Boot, system and data deployment

The initial RAM archive contains only static bootstrap and recovery programs.
Stage-1 mounts the independent system volume temporarily at `/system`, then
switches that filesystem to `/` **before** loading system init. The default system
volume is read-only EROFS; writable persistent files live on the FAT volume at
`/data`. Formal programs and shared libraries are not embedded in the bootstrap
archive. Plain `make`, `make rebuild` and `make run` use this deployment.

The kernel mounts the supplied RAM archive as `/` and starts `/init` through
`/lib/userspace-loader-hyper-<arch>`. Those are its only bootstrap path
conventions. The temporary `/system` path, `/boot`, stage-1, and the service handoff
are userspace policy. The system environment has no stage number;
`component-manager` is its root supervisor. The Pi firmware's FAT boot partition is separate from the
Native `/boot` mount.

## Files and dependencies

[`bootstrap/stage1`](../bootstrap/stage1/README.md) owns static bootstrap
programs. Formal commands live in `app/` and background providers in `svc/`, and their reusable
implementations live in `lib/`. Bootstrap does not link these formal libraries.
It may use the Native kernel-facility substrate, static Rust std and third-party
Cargo crates. Supervisor, format and I/O implementation modules belong to their
own bootstrap executable packages; there is no `stage1/lib` package graph.

The initial archive directly contains `/init`, `/svc`, `/bin`, `/etc`, `/vm`,
`/lib64` and `/lib -> lib64`. After switching roots, the layout is:

```text
/bin/                              formal shell and Native commands
/svc/                              component-manager and formal services
/etc/components/*.json             reusable component declarations
/etc/hyper/components.json         instance-tree root and configuration directory
/etc/hyper/startup.json            finite provider-adoption policy
/etc/fstab                         later mounts requested by /mounts
/lib64/ld-hyper-<arch>.so           formal executable loader and dynamic linker
/lib64/<arch>-hyper-hyper/          shared libraries
/lib -> lib64                      library-directory alias
/boot/                             original archive until bootstrap retirement
/tmp/                              separate writable RAM filesystem
/data/                             persistent FAT volume mounted in the formal environment
```

The system image includes empty `/boot`, `/tmp` and `/data` mountpoint directories,
so preparing the transition does not require writes to EROFS. Stage-1 switches the
mounted system filesystem to the namespace root. The old archive becomes `/boot`;
the temporary `/system` is not a second path to the formal root. After accepting
supervision, component-manager commits the root transition. System-startup mounts
a fresh RAM filesystem at `/tmp` before preparing replacement providers. After provider adoption,
bootstrap-retire stops predecessor processes and detaches `/boot`. Its empty
mountpoint directory remains. The last kernel owner of an archive-backed file or
page releases the corresponding physical backing.

Bootstrap launchers retain fixed `process.environment-directory` capabilities
for their own files as the archive moves to `/boot`. Their separate namespace
root capability follows the root switch. A fixed file or subdirectory capability
keeps its existing identity and confinement; root switching does not reopen files
or change their contents. The filesystem itself and its mounted descendants move
as one namespace transaction. Ordinary kernel RAMFS and namespace mechanisms do
not embed any of these deployment paths.

Formal init is opened only after the root switch. It and every formal child use
the independent system filesystem for executable and library lookup. Core services
use the static bootstrap loader. Formal programs, including static executables,
use the delegated `/lib64/ld-hyper-<arch>.so`; dynamic linking also uses the
`/lib64/<arch>-hyper-hyper` Directory capability. A dynamic ELF's canonical
`PT_INTERP` is `/lib64/ld-hyper-<arch>.so`. `ldd` inspects that path literally:

```sh
ldd /bin/vmm
ldd --tree /svc/vm-runtime
```

The default bootstrap selection contains init, console-raw, fallback-sh,
io-runtime, fs-backend, fs-erofs, fs-fat, mount and umount. `APP_BOOT_COMPONENTS`
selects staged component names. Each bootstrap executable must be static; the
SDK driver and final ELF checks reject an interpreter or shared-library dependency.
The default keeps the static FAT worker available for recovery, while the formal
image also contains its independent FAT worker. Removing `fs-fat` from the boot
selection does not remove the formal `fs-fat-system` component.

Build artifacts retain the selected debug information policy. Packaging strips
only debug sections from temporary copies. The formal executable and transitive
DSO closure is checked for architecture, SONAME and symbol compatibility before
publication.

## Supervision handoff

Stage-1 validates the core manifest, starts raw console and storage services,
and waits for mounts in the archive's `/etc/fstab`. The I/O VM starts independently
of the business VM manager; keeping Native storage alive does not require a
business fleet or provisioning connection. Bootstrap contains no terminal service
or line discipline, and launches its byte-stream fallback shell only on failure.

Once `/system` is an independent mount, stage-1 prepares a reversible root
transition. It starts `/svc/component-manager` in a separate candidate task
group. The manager reads the component tree and prepares system-startup, which
validates the offered capability inventory and migration policy. One capability-channel rendezvous transfers core Process
supervisors and their management inventory. That transfer is the supervision
commit point. Before commit, failure retains stage-1 ownership and rolls the
namespace back for recovery. After commit, the formal environment owns failure
handling and retirement timing; stage-1 does not restart its fallback shell.

Component-manager forwards domain resources to system-startup, which replaces bootstrap console, I/O and filesystem providers with formal processes. The running I/O VM and mounted filesystem identities survive adoption:
the VM does not reboot and volumes are not remounted. New providers start before
predecessors retire. A failed adoption retains still-needed predecessor resources.
The finite coordinator applies the formal fstab through a mount job, starts
formal services and configures the VM fleet. An independent retirement job
runs after confirmed provider adoption; successful exit publishes the
bootstrap-retired progress fact. Data mount failures are handled in the formal environment and do not
return to stage-1. See the [handoff contract](bootstrap-handoff.md) for transfer
records, ownership and retirement conditions.

The boot `/etc/hyper/init.json` selects the independent root and successor:

```json
{
  "system-root": "/system",
  "put-old": "/boot",
  "system-init": "/svc/component-manager"
}
```

`system-root` is resolved before the switch; `put-old` and `system-init` name
locations in the new namespace. Formal `/etc/hyper/components.json` selects the
component configuration directory, root declaration and startup coordinator; it
does not specify another root switch. See
[service management](services.md) for the JSON schema.

Namespace mutation requires `MANAGE_NAMESPACE | WRITE | EXECUTE` on a namespace
root Directory. Ordinary shells and format workers do not receive that authority.
Stage-1 recovery can repair mounts, but does not automatically repeat the handoff;
there is no `initctl` retry command. A subsequent system startup attempt requires
a reboot. Critical failures after committed handoff follow formal supervisor policy.

Formal `session-manager` supervises `terminal-service` and `/bin/sh`. The static
`svc/fallback-sh` is a stage-1-only recovery path without clap, history, pipelines
or redirection. Neither shell searches `svc` for commands. Background services
receive independent bounded log channels; the raw console mirrors these logs
alongside interactive output. Replacing raw console does not add terminal policy
to bootstrap.

## Runtime filesystem policy

Each environment owns a separate fstab. The archive's `/etc/fstab` mounts only
what stage-1 needs:

```fstab
PARTLABEL=hyper-system /system auto ro 0 0
```

The independent system filesystem's `/etc/fstab` contains later mounts:

```fstab
PARTLABEL=hyper-data /data auto rw 0 0
```

Stage-1 never reads the formal table. The formal mount job requests it after taking
supervision. No stage-number flags are needed and the kernel parses neither table.
Sources select authorized GPT partitions with `PARTUUID=` or `PARTLABEL=`, not
Linux device paths. HypeR reads GPT and grants bounded block ranges to the
filesystem manager. The Linux I/O backend does not resolve logical volume names.

The manager identifies media by magic and launches a separate format-worker
process for each mount. `auto` selects the detected format; `fat`, `vfat` and
`erofs` require a matching format. EROFS requires `ro`. Supported options are
`defaults`, `ro`, `rw`, `auto`, `noauto` and `nofail`. The final fields must be
`0 0`; fsck and dump scheduling are not implemented. Entries run in file order.
Required bootstrap failures enter recovery; required formal failures are reported
without preventing unrelated formal services. `nofail` makes a mount optional;
`noauto` leaves it for an explicit request.

Each manager reads `etc/hyper/filesystem.json` relative to its own environment:

```json
{"worker_directories": [{"environment": "svc"}]}
```

The bootstrap manager finds static workers in its archive; the formal manager
finds formal workers in the system filesystem. Explicit `namespace` search paths
are also supported. A format needed to mount the system root must have its worker
available before switching roots. Missing workers and unsupported formats produce
runtime errors; the builder does not impose a filesystem-provider dependency graph.

```text
mount                                      list managed mounts
mount -a                                   apply the active environment's /etc/fstab
mount /data                                mount one fstab target
mount -T /data/custom.fstab -a              apply an explicit table
mount -t auto -o ro PARTLABEL=backup /data/backup
umount /data                               sync and detach an idle mount
```

Unmount rejects open files, protected mappings, directory capabilities, active
lookups and nested mounts. There is no forced or lazy unmount. Successful unmount
drains RPC, retires the mount/cache generation, stops the worker and returns its
exclusive block capability. A dead worker or unfinished block transaction is not
silently reactivated. See [filesystem services](filesystem-services.md).

## Build and customize

```sh
make
make run
make rpi5-sd
```

QEMU output defaults to `target/board/qemu/`; Pi 5 output defaults to
`target/board/rpi5-native/`. `bootstrap.cpio` contains only bootstrap files,
`system-root/` is checked formal staging, and `disk.img` contains separate system,
data and guest volumes. System files are not packaged inside the boot archive.
The removed `BOARD_SYSTEM_LAYOUT` option is rejected rather than silently ignored.
`make rpi5-bringup` is a diskless static recovery diagnostic and never launches
a formal system environment.

The default EROFS image uses uncompressed core format, 4096-byte blocks, no xattrs,
a fixed timestamp and cleared UUID. `EROFS_MKFS` selects an existing host
`mkfs.erofs`; the build does not install a global tool. FAT system partitions are
also supported when their payload contains no unsupported symbolic links.

`disk.img.system.json` records the checked system payload. This detects mismatched
builds, not runtime tampering. Ordinary `make` refreshes a changed EROFS system
partition after validating both GPT copies and capacity. It publishes a sibling
disk image atomically while preserving every byte outside that partition. An
unchanged payload leaves the disk untouched. Stop QEMU before refreshing its disk.
Writable FAT system volumes and changed partition layouts need explicit replacement.
`make run` only checks existing artifacts. `make rebuild` replaces the whole disk,
including persistent data and guest volumes. Pi replacement requires
`BOARD_IMAGE_REPLACE=--replace`.

Partition content belongs to `disk.partitions`; access and mount policy are
independent, for example:

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

Normal board images require a generated independent system partition. Each Native
partition consumes one of eight I/O data clients shared with guests. Client zero
is the owner-only physical block control channel. Labels are not reserved; change
their selectors with them, or use PARTUUID to distinguish equal labels. See
[board storage](board-storage.md) for image inputs and partition schema.

Functional checks in `tests/qemu/verify-boot-system.py` exercise root switching,
provider takeover, retirement and recovery. Parser and EROFS fixture checks run
with `make app-test APP_TEST_WORKSPACE=bootstrap/stage1 APP_TEST_PACKAGE=hyper-fs-erofs`.
VM-manager isolation checks use private disks and test-only stand-ins. These tests
establish lifecycle and filesystem behavior, not storage throughput or physical
durability.
