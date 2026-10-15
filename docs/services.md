<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Components and service supervision

The formal root is `component-manager`. There is no intermediate formal init and
no service dependency or target engine. Components form a static instance tree;
capability routes determine which services can connect. Domain-specific startup and
logging remain separate processes; the root manager renders early boot status. Bootstrap does not link any
of these formal libraries.

| Program | Responsibility |
| --- | --- |
| `component-manager` | Validate the instance tree and routes, accept bootstrap ownership, own component Process supervisors, activate providers, report lifecycle state and display early startup progress. |
| `system-startup` | Validate the offered resources, adopt existing console/I/O/filesystem providers, mount formal volumes, provision the initial VM fleet and arrange bootstrap retirement. It exits after successful migration. |
| `journal` | Collect attributed byte streams, retain bounded records in RAM, serve queries and mirror logs to raw console. |
| `bootstrap-retire` | Stop transferred predecessor processes, drain their console, join teardown and normally unmount the old archive. |
| `session-manager` | Own per-console terminal services and interactive clients. |

These programs install in `/svc`; foreground tools install in `/bin`. This is a
deployment convention, not an executable-path restriction. VM and filesystem
managers retain their domain workers. The component manager does not link device,
filesystem, terminal, journal retention or VM provisioning implementations.

## Composition

The root manager reads `/etc/hyper/components.json`:

```json
{
  "configuration_directory": "/etc/components",
  "root": "root",
  "startup": "/startup"
}
```

The directory is configurable. Each `NAME.json` contains a reusable declaration
whose `name` equals `NAME`. A parent's `children` instantiate declarations under
local names. `/` identifies the root realm; `/tools/indexer` identifies one
instance, even when another instance uses the same declaration. Realms may have
no executable. Files are loaded once; unknown fields, recursive trees, invalid
routes and size limits reject preparation before bootstrap transfers ownership.

A parent explicitly offers a child's exposed protocol to another child:

```json
{
  "name": "tools",
  "children": [
    {"name": "indexer", "component": "indexer"},
    {"name": "client", "component": "client"}
  ],
  "offer": [{
    "name": "index", "to": "client", "from": "#indexer", "source": "query",
    "kind": {"type": "protocol", "protocol": "example.Index"}
  }]
}
```

The provider declares `capabilities` and `expose`:

```json
{
  "name": "indexer",
  "program": {"image": "/svc/indexer", "startup": "lazy"},
  "capabilities": [{"name": "query", "kind": {"type": "protocol", "protocol": "example.Index"}}],
  "expose": [{"name": "query", "from": "self", "source": "query", "kind": {"type": "protocol", "protocol": "example.Index"}}]
}
```

The consumer declares `use` with its local name `index` and matching protocol.
Sources are only `self`, `parent` or `#immediate-child`. Every intervening realm
must explicitly expose/offer the capability. An absolute instance path is never
connection authority. Routes also support Native objects with `object_type`,
Native `rights` bits and a consumer startup `purpose`; each boundary can narrow
rights. `transfer: "move"` permits only one consuming use and prevents restarting
that component with the consumed handle.

`system-startup` may install realm-owned objects through explicit `exports` in
its policy, each naming `realm`, `name` and `source`. The matching realm
capability supplies the rights ceiling. Supported sources are `component.observe`,
`component.control`, `bootstrap.root-directory`, and the task, object, memory and
CPU inspectors. Exported handles are distinct duplicates; routes determine
whether a consumer receives another duplicate or consumes the exported handle.
The root manager does not infer authority from a filesystem path or component name.
Persistent object capabilities belong to non-executable realms. Prepared instances
receive their object grants in the coordinator's construction transaction; their
declarations may use routed protocols but cannot silently override those grants.

Component-manager reports startup progress alongside ordinary service logs. A
single opening line identifies the manager, architecture and boot time. Complete
status lines show component descriptions, observed durations and coordinator
milestones, followed by a compact lifecycle summary. `START`, `RUNNING`, `DONE`
and `FAILED` describe process state, not protocol readiness. There is no animation,
row rewriting or console-wide log suppression. Timings begin when the manager
observes a launch; completion rows include the observed run duration. They are
not hardware or guest boot benchmarks. The retirement service remains visible
through completion. Coordinator exit or a 60-second deadline ends the temporary
report without changing service readiness, logging or resource ownership.

Formal io-runtime, fs-backend and vm-manager receive separate stdout/stderr
channels registered with journal before launch. Their children inherit those
streams: I/O VM output belongs to io-runtime, filesystem worker output to
fs-backend, and VM runtime output to vm-manager. A normal formal service cannot
silently fall back to raw-console registration if journal has not started. The
journal's own startup diagnostics use explicit raw-console channels to avoid a
circular dependency. Coordinator stdout/stderr use bounded temporary relays until
journal registration, so early errors cannot block the process that starts it.
Bootstrap services retain their independent raw log channels until retirement.
The manager supervises components; session-manager owns terminal and shell
lifetimes. See [Native terminals](terminal.md) for the initial session handoff.

## Activation and lifetime

- `startup` defaults to `lazy`; `eager` requests activation after launch resources
  and logging are available. Reading status or inspecting routes never starts a
  component.
- A client uses its private connector and local capability name. The first
  connection starts a stopped provider. Concurrent connections share one launch
  generation. The provider publishes its declared protocol and accepts sessions
  on its outgoing endpoint. Publication is separate from Process start.
- Once connected, client and provider communicate directly. The manager is not
  in the request/reply data path. It queues at most 64 pending connections,
  at most eight per consumer, with a maximum 60-second connection deadline.
  Missing publication, provider failure and queue exhaustion return errors.
- Endpoints are tied to instance and generation. A stale connector cannot publish
  or connect for a replacement. Existing sessions are not replayed or repaired
  across provider death; callers must reconnect and handle application recovery.
- `restart` is `never` (default) or `on-failure`, with `restart_limit` (default 3,
  maximum 10) and `restart_delay_ms` (default 1000). An explicit restart resets
  the retry budget. Exit zero reaches `completed`; it does not create an eager
  respawn loop. A later authorized start or connection can activate it again.
- Start/stop/exit decisions have one owner. Replacement waits for Process retirement,
  the construction worker and the owned TaskGroup to finish. A timeout cannot
  substitute for teardown. An observed Process exit immediately withdraws its
  published routes and enters `stopping` while descendants retire. A subsequent
  stop cancels an outstanding restart.
- `resident` permits the application to use memory locking; it does not pin the
  application automatically. Critical migration participants explicitly lock
  their current mappings before the relevant ownership rendezvous.

Limits are 64 instantiated components, depth 12, 16 KiB per declaration and
128 KiB total configuration. There are at most 32 uses per declaration and 16
arguments per program. The tree is static: dynamic collections, live reload,
idle auto-stop and automatic manager recovery are not implemented.

## Bootstrap adoption

Stage1 still launches the configured successor through its existing handoff ABI.
The shipped successor is `/svc/component-manager`. It prepares the configured
startup coordinator, waits for its resource-policy validation and locked mappings,
then accepts the original atomic inventory and commits the root transition.
It forwards domain resources to the coordinator through a separate private
committed-startup rendezvous. This leaves the kernel and bootstrap contract intact.

`/etc/hyper/startup.json` identifies provider roles, journal, mount/retirement jobs
and initial applications. Composition translates board deployment grants into this
finite migration policy. `prepared: true` declarations require the coordinator to
submit an unsealed ProcessBuilder and its TaskGroup. The manager adds private
component endpoints, starts the process and retains its supervisor; the coordinator
receives only a WAIT observer. Ordinary components cannot access this installer.
Every prepared instance needs exactly one entry in this policy; every routed
object needs a matching startup export. These checks run before ownership commit.
Prepared construction honors the declared arguments and `resident` permission;
adoption providers must permit residency because their protocol requires it.
The retirement role prepends its process count before configured arguments.
An initial application's `interactive: true` marks the single terminal readiness
participant, independent of its image path. The coordinator waits for its READY
notification before publishing `interactive-ready`.

The current console, I/O, filesystem, journal, session and VM-manager instances
use prepared construction. They cannot be stopped or restarted through generic
control: their transferred endpoints or live domain resources need an explicit
ownership protocol. This restriction is checked even after a failed first start.
The startup coordinator itself cannot be recreated from its consumed inventory.
After adoption, the I/O and filesystem providers remain ordinary managed instances
for status, exit observation and attributed stdout/stderr. The stop/restart
restriction does not exempt them from supervision or journal queries.

Startup publishes diagnostic facts such as `core-providers-adopted`,
`filesystems-mounted`, `interactive-ready` and `bootstrap-retired`. These are not
named dependencies and cannot start other components. The coordinator performs
the finite sequence; successful retirement alone publishes `bootstrap-retired`.
If ownership becomes uncertain, it retains known owners and reports failure.
Neither it nor the manager pretends that a committed MOVE rolled back. Stage1
recovery remains available only before the original handoff commit.

## Administration and logs

```sh
componentctl
componentctl status /io-runtime
componentctl routes /session-manager journal
componentctl start /tools/indexer
componentctl stop /tools/indexer
componentctl restart /tools/indexer
journalctl -n 100
journalctl -c /vm-manager -f
```

Control acknowledges acceptance; query status for completion. Read-only observation
and administrative control use distinct capabilities. The trusted shell delegates
these only to resolved `/bin/componentctl`; `/bin/journalctl` receives its direct
journal query session. Component status includes path, generation, Process KOID,
restart count, published protocols and bounded detail text. Route inspection shows
each realm boundary.

Ordinary processes receive stdin at EOF and independent stdout/stderr byte writers.
Programs using std I/O need no framework adaptation. Trusted supervisors register
sources; printed text cannot forge attribution. Records identify component path,
execution generation, stream, monotonic time and sequence. Legacy internal domain
log registrations retain their supplied labels and generation zero.

Retention is volatile: 1 MiB/4096 records globally and 128 KiB/512 records per
component. Lines split at 4096 bytes; partial lines flush periodically and on EOF.
Admission and per-tick work are bounded. Queries use bounded framed byte replies;
clients cannot retain journal-sponsored reply VMOs. The CLI escapes terminal control
characters, reports retention loss and uses a fixed upper cursor for initial lists.

Logs mirror to raw serial during startup and after terminal activation, alongside
ordinary kernel logs. The mirror uses each source's line framing, rather than
mixing individual stdout/stderr write fragments. Partial records also terminate a
physical line, while retained records preserve their continuation flag. Logs may
interrupt the interactive display. A full mirror queue drops bytes and
counts them without blocking retention; producer queues provide bounded backpressure.
This is not a durable audit log. Panic output remains an independent kernel path.

## Verification

`make app-test APP_TEST_PACKAGE=hyper-component-config` checks tree/route policy;
`hyper-component-manager` checks lifecycle arbitration; `hyper-system-startup`
checks adoption/provisioning policy; `hyper-journald` checks retention.
`make test-services` exercises nested routes, eight concurrent lazy connections,
separate observation/control authority, stdio, connection deadlines, restart bounds
and retirement using actual Native processes. See [bootstrap handoff](bootstrap-handoff.md)
and [boot layout](boot-layout.md) for the filesystem and ownership contracts.
