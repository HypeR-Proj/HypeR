<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# `mount` and `umount`: Native volume administration

[mount](https://github.com/HypeR-Proj/mount/tree/main/) lists active managed volumes without arguments. `mount -a`
applies automatic `/etc/fstab` entries; `mount /data` or
`mount PARTLABEL=hyper-data` selects one entry there. An explicit `-T` path
selects another table; `/boot/etc/fstab` exists only while bootstrap is retained. `mount -t auto -o ro PARTLABEL=backup /mnt/backup` selects an authorized
GPT partition and target. Types are `auto`, `fat`/`vfat`, and `erofs`; EROFS requires
`ro`. The manager identifies media before selecting an installed worker, and
reports unavailable volumes, format mismatches and missing workers separately.

[umount](https://github.com/HypeR-Proj/umount/tree/main/) synchronizes and detaches one idle target. Open files,
directory/cwd handles, active lookups or nested mounts return Busy and preserve
the existing mount. It has no forced or lazy mode. A clean shutdown returns the
worker's exclusive block owner to the manager for a later mount; a crashed
worker is not silently reactivated.

The formal manager reports unexpected worker exits through its journal stream.
`componentctl status /fs-backend` shows the failure summary; `mount` includes
each worker's process KOID and terminal reason. A failed volume does not stop
the manager or its other workers.

Both commands need filesystem control authority, not raw device access. The
formal session manager passes it to its shell, which delegates only WAIT|WRITE
to `/bin/mount` and `/bin/umount`. Independent static recovery
copies remain in bootfs for failures before system takeover. See
[filesystem services](../../design/storage/filesystem-services.md) for the protocol and
[boot layout](../../architecture/boot-layout.md) for supported fstab options.
