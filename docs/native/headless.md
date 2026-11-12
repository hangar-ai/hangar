# Headless Hangar

Goal: a Hangar that runs its workspaces with no window at all, on a server,
the PC off, driven from Discord (discord-connector.md) or from a desktop that
connects to it. The orchestration must not depend on the front end.

## Where things stand

The logic of the workspaces lives in `hangar-app`, about 48,000 lines:

| Package | Lines | What it is |
|---|---|---|
| `state` | 6,200 | The persisted state, its store, and the window's read-models |
| `panes_logic` | 10,900 | What each pane is doing (the bridge's reports, activity, hibernation) |
| `control` | 8,300 | The agents' operations (MCP): panes, workers, deliveries, previews |
| `orchestration` | 8,800 | Workers, events to the orchestrator, briefings, the manager |
| `mobile` | 7,600 | The desk: the calls the chat view and the phone make |
| `nodes` | 6,400 | Driving nodes over SSH |

It is already written against seams rather than against the window: the
engines run on tokio, read a `Snapshot` of the model and write through
`ModelCmd`s that the window applies (`control::port::AppPort`), and the UI
edges are few and in known places: each package's `mod.rs` (iced's `Task`
and `Subscription`), its `live.rs` (the port over the app's `Services`), and
`control/writes.rs` (applying the writes to the `Model`). A node today runs
the terminals, the boards, the memory and the deliveries; the control
requests of its agents go to the desktop that drives it, and without one
they are refused.

## The plan

1. **`hangar-engine`, a crate with no window.** No iced, no theme, no
   widgets: its `Cargo.toml` is the guard. The packages move there one at a
   time, leaves first, the app keeping their iced glue and re-exporting them
   where they were, so nothing else changes and every test passes after each
   move:
   1. the persisted state and its store (`state`): **done**;
   2. what the panes are doing (`panes_logic`): its state machines and their
      read-model, **done**; its clock (iced subscriptions) and the glue that
      feeds it from the `Model` stay in the app until step 2;
   3. the agents' operations (`control`): **done**; the model writes they ask
      for (`ModelCmd`) are still applied by the app (`control/writes.rs`)
      until step 2;
   4. the orchestration engines (`orchestration`): **done**; their runtime
      (the tokio task that owns them, `runtime.rs`) and their ports over the
      app's services (`live.rs`) stay in the app until step 2;
   5. the desk (`mobile`'s calls, without the phone's server): **done**; its
      sources over the app's services (`mobile/live.rs`) and the iced glue
      stay in the app.
2. **A host for the engine without a window**: the store and the
   read-models in one place, saved to disk on tokio's clock, the core's
   events fed to it, the writes applied there (`hangar_engine::host`):
   **done**. The orchestration runtime is one, shared: the app hosts it with
   its window's ports, the engine's host with its own.
3. **The node runs the engine for its own workspaces** and is their
   authority: their state saved on the node, its agents' control requests
   answered there: **done**. A desktop that connects shows them on the
   node's card; the remote workspaces it drives itself stay as they were
   (moving them to the node is still to do).
4. **The Discord connector in the engine** (discord-connector.md): it runs
   where the engine runs, on the node for its workspaces with the PC off:
   **done** (`crates/hangar-discord`).
5. **A node that starts on its own**: its settings (the bot's token among
   them) on the server, `hangar-node serve` with no desktop, workspaces
   created from Discord: **done**.

## Rules for the moves

- A module moves with its tests; an app test that needs the window stays in
  the app.
- The app re-exports what moved under its old path (`crate::state::types`
  is `hangar_engine::state::types`), so a move touches the moved files and
  the package's glue, not its users.
- What the engine needs to know of the app is a trait the app implements
  (the ports), never a dependency on it.
- Each move is one commit on main that builds, passes clippy and every test.

## Step 1.1: the persisted state

`hangar_engine::state`: `types`, `store` (`Store`, `StoreAction`), `layout`,
`roles`, `folders`, `pane_names`, `unread`, the lenient reader and what a
save writes (`persist`). The app's `Model` holds a `Store` beside the
window's own state (`ui`, `live`) and delegates to it; hydrating mounts the
active workspace and opens the model on the app's side. The default theme
is spelled in the engine (`DEFAULT_THEME_ID`), a test holding it equal to
hangar-theme's.

## Step 1.2: what the panes are doing

`hangar_engine::live`: the read-model types the engines share (activity,
insight, the mod's reports, rate limits). `hangar_engine::panes`: the state
machines (activity, insight, hibernation, sessions, branches, delivery
stages) and their `Live`, which the app keeps as `Model.live.panes_logic`.
The window's part of attention (which window has the system's focus) is
`Model.ui.system_focus` now, not the engine's. `Live`'s fields are open to
the host that drives it; the app's `panes_logic` package remains that host
(its `update`, ticks and subscriptions) until the engine has a host of its
own. Helpers that put the engine in a state for a test are behind the
engine's `testing` feature, which the app's tests turn on.

## Steps 1.3 and 1.4: the agents' operations and the orchestration

They depend on each other (the events of the orchestrator name the
deliveries' outcomes, the deliveries post events), so they moved together,
after the pieces they read: the `Snapshot` (taking it from the `Model` stays
the app's), the account rules, where panes run, the git request, the
preview access types and the record of the last delivery checks. The dev
command names a palette command by id, the host saying which ids it knows
(`AppPort::knows_command`), and its capture size as numbers.

`hangar_engine::control`: the dispatch (`run`), the port (`AppPort`,
`ModelCmd`, the hooks), the operations (panes, previews, kit tabs,
workspaces, manager, deliveries, board relay) and the fake port their tests
run against. `hangar_engine::orchestration`: workers, events, the manager's
feed, briefings, projects, the sweep of stale worktrees. What stays in the
app is the host: building the port over its services (`control/live.rs`,
`orchestration/live.rs`), applying the writes to the `Model`
(`control/writes.rs`), the requests of a node's agents (`control/remote.rs`),
the runtime and the iced glue. The delivery harness the tests share is
`hangar_engine::control::testing`, behind the `testing` feature.

## Step 1.5: the desk

`hangar_engine::mobile`: what a surface away from the window asks of the
panes (the chat view's calls, the phone's, a connector's): conversations,
sending, answers, permissions, plans, the queue, deliveries, apps, the
activity log, push texts and the statistics. With it the statistics' model
and calendar (`hangar_engine::stats`). The transcript of a pane on a node is
asked of the host (`Sources::node_chat`): the desktop reads it over its SSH
connection, and a host that drives no nodes has none to reach. Hangar
Mobile's records (the activity log's lines, the shared files) are in
hangar-core's always-built `mobile_data`, its server (`remote`) staying
desktop-only.

With this, step 1 is done: what runs the workspaces no longer depends on the
window. What remains in the app around each package is its host: the ports
over its services, the writes applied to its `Model`, its runtime and its
iced glue. Step 2 makes a host of the engine's own.

## Step 2: a host with no window

`hangar_engine::orchestration::runtime`: the tokio task that drives the
orchestration engines, generic over what hosts it (`RuntimeHost` makes the
ports its engines read and act through). The app's `AppRuntime` makes its
`LivePort`s over the window's services, on a workspace's node when it lives
on one; the engine's host makes its own. The engine's hook on the control
bus (`OrchestrationHooks`) is the runtime's handle.

`hangar_engine::host::EngineHost`: the engine with no window.

- **State.** Loaded from its file (`read_state_file`, lenient as the app's
  boot; a file it cannot read is an error, never an empty state), saved
  400 ms after the last change (`SAVE_DEBOUNCE`), whole, to a temporary file
  renamed over it, and once more on the way out (`shutdown`).
- **Writes.** The engines' `ModelCmd`s applied to the store at once
  (`host/writes.rs`): panes added, closed, restarted and laid out again as
  on the desktop; deliveries counted; workspaces created and removed. What
  only a window does (detach, bring back, a palette command) is refused;
  showing a pane is nothing to do. An agent's announcement is `News`.
- **Panes.** Every workspace is open, and every pane of it has a shell
  (`host/launch.rs`): started through `Terminals` with the bridge mod, the
  account, the build cache's variables and, for a workspace that chose its
  MCP servers or a pane with a role, core's `--mcp-config` file; the agent's
  command typed once the shell has printed and fallen quiet, with its
  session resumed when its transcript is there and the orchestration's
  permission mode for a role. A pane removed from the state is killed; a
  shell that ends stays down until the pane is restarted (a new id).
  `CorePtys` runs them through core's PTY manager, each with a screen
  (`host/screen.rs`: alacritty's emulator, no widget) that answers what a
  program asks of its terminal, and that `pane_read` and the trust question
  read.
- **Events.** The mod's reports, the queue and the deliveries' stages go to
  the panes' state machines; the reports, the shells' ends and the boards
  written to the orchestration runtime. A machine's notification is `News`.
- **Requests.** `answer(op, args, internal)` runs an agent's request through
  the engine's dispatch over `HostPort`, and Hangar's own (a briefing, a
  phone's call); `serve_control` answers every request core is asked, for a
  host alone on its machine.
- **Ports.** `HostPort` (the control port: no preview, no window, no node),
  `HostPorts` (the orchestration's: boards, git, memory and the queue on
  this machine).

What stays the app's own around the engine is the window: its terminals'
widgets, the hub, the preview, the windows and their focus, notifications;
and what reaches its nodes.

## Step 3: the node runs its own workspaces

`hangar-node serve` hosts the engine (`hangar-node/src/own.rs`): an
`EngineHost` whose state is `~/.local/state/hangar-node/state.json` and
whose terminals are the node's own panes (`NodeTerminals` over `Panes`),
each with a screen. Their history and streams are the node's, so a desktop
streams them as any pane of the node.

- **Authority.** core's control requests are looked at first by the node
  (`control::watch`): one whose caller is a pane of the node's own (an
  agent's `caller.paneId`, a briefing's `paneId`) is answered there, through
  the engine's dispatch, with no desktop connected; the others go to the
  desktop that drives the node, as before, or are refused when none does.
  What the node answers is kept off the desktops' events (`/node/events`):
  a desktop would answer it too, about a pane it does not know, and core
  takes the first answer. An agent in no pane (a Claude Code started by
  hand on the server: it speaks with the endpoint file's token, which
  names no pane) is the desktop's when one drives the node, and the node's
  own workspaces' otherwise.
- **A workspace is one change.** `add_workspace` writes the workspace and
  its orchestrator's role together: told of the pane before its role, the
  panes would start it as a plain Claude Code, without the `--mcp-config`
  file that gives it Hangar's tools, and nothing on a host with no window
  repairs a pane that runs. A pane that leaves the state while its shell is
  still starting has that shell taken down by the start itself (each start
  holds its pane by a number): the kill that comes with the removal finds
  no shell yet.
- **Starting.** The host is loaded before the API listens: a state file it
  cannot read stops the node rather than letting a save write over it. Its
  last save runs on the way out.
- **The API.** `GET /node/own` lists the workspaces with their panes, their
  role and what they are doing; `POST /node/own/workspaces {name, cwd,
  orchestrator?}` makes one in a folder of the node, with one Claude Code
  pane, its orchestrator unless told otherwise; `DELETE
  /node/own/workspaces/{id}` removes one, its shells with it; `POST
  /node/own/panes/{id}/send {text}` queues what the person says for a pane's
  session.
- **For a desktop that mirrors them.** A desktop keeps a copy of these
  workspaces in its own state and shows them like its own, the node staying
  their owner (`hangar-engine/src/state/hosted.rs` says which fields are the
  node's and which stay the copy's presentation). `GET /node/own/state`
  answers `{boot, revision, workspaces, activity}`: each workspace whole, as
  the state holds it; `revision`, which moves when that state does and never
  for an agent's activity alone; `boot`, made when the daemon starts, since
  the revisions start again with it; and `activity`, by pane id (`working`,
  `waiting`, `yours`, `idle`), beside the panes, not in them. `GET
  /node/events` sends `{"event": "own:changed", "revision"}` when the
  revision moved. `POST /node/own/workspaces/{id}/ops {ops}` applies what
  the person changed on the copy, in order, all or none, and answers
  `{revision}`: `putPane` (a pane under the id the desktop chose),
  `closePane`, `respawnPane` (`from`, `to`), `patchWorkspace` and
  `patchPane` (the fields named, `null` emptying one), `removeWorkspace`.
  Applied twice, they change nothing the second time.
- **Handing a workspace over.** `POST /node/own/adopt` takes a whole
  workspace, with its ids: one a desktop ran on this node, or one it has
  just made for it. It is checked first (its folder is one of the node's,
  its ids are usable and free) and refused with 400 before anything is
  touched. The shells that run under its pane ids, which a desktop started,
  are then taken down, and the node starts its own under the same ids, each
  Claude Code resumed on its pane's session. What was the desktop's
  presentation (its folder in the navigator, its window, its views, its
  kits) is left out, and a pane whose name is taken on the node gets a free
  one. A workspace the node already has is answered as done, nothing
  restarted.
- **The node's panes are the node's.** `POST /node/panes` and `DELETE
  /node/panes/{id}` answer 409 for a pane of the node's own: a desktop
  that started or ended one would run an agent twice, or kill one the node
  answers for. A resize of such a pane also resizes the screen the engine
  reads it from.
- **On the server.** `hangar-node workspaces`, `hangar-node workspace add
  <folder> [--name <name>] [--no-orchestrator]`, `hangar-node workspace
  remove <id>` and `hangar-node send <pane> <text…>` ask the running daemon.
- **On the desktop.** A connected node's card lists the workspaces it runs
  itself, with each pane and what it is doing, read every 5 s.

Tested on the binary itself (`hangar-node/tests/node.rs`): a workspace
made through the API, its orchestrator's shell started and a Claude Code
typed in it (a script standing in for it) whose request through Hangar's
MCP server the node answers with no desktop, the state kept across a
restart, and the workspace removed with its shell; the same from the
command line; a workspace handed over above a shell a desktop started under
the same id, which comes back as the node's on its session; the ops, the
409s, and `own:changed` told for a change to the state and not for an
agent's activity.

## Step 4: the Discord connector

`crates/hangar-discord` (discord-connector.md): a bot beside an engine
host, run by the node's daemon for the workspaces it runs itself.

## Step 5: a node on its own

On a Linux server, with nothing else than the binary:

```sh
hangar-node service install          # or: hangar-node serve --detach
claude                               # once: sign Claude Code in on the server
hangar-node discord setup --guild <server id>   # the token on standard input
#   prints the invite link, and the code to send: `pair <code>`
hangar-node workspace add ~/projects/shop       # or /workspace on Discord
```

- **The settings on the server.** `~/.local/state/hangar-node/discord.json`
  (0600): the token, read from `HANGAR_DISCORD_TOKEN` or standard input,
  never an argument; the server; who is paired and the pairing code.
  Another server or another token unpairs. `hangar-node discord status`
  says what is set and whether the bot runs; `hangar-node discord off`
  forgets the token. The invite link names the bot's id, read from its
  token, with the scopes and permissions the connector needs.
- **The daemon runs the bot** beside its own workspaces as soon as the
  settings are complete, and again when they change (`POST
  /node/discord/reload`, which `setup` and `off` call); a bot that stops
  (Discord out of reach, a token refused) is tried again, waiting longer
  each time, up to five minutes.
- **A service.** `hangar-node service install` writes a systemd user unit
  running `hangar-node serve --supervised`, started again however it ends
  (`Restart=always`: the desktop stops the node with a plain `kill` before
  it upgrades it, a clean end), and enables it; `loginctl enable-linger`
  keeps it after the person logs out. `--supervised` is for the unit: where
  a daemon already runs outside the service (the desktop starts one over
  SSH, `serve --detach`), a plain `serve` leaves, which would end the unit
  with nothing left to start the node again; the supervised one waits
  beside that daemon, its agents untouched, and becomes the node when it
  stops. A node that stops does not wait for what still waits on Discord.
- **The token typed.** At `discord setup`'s prompt it is not shown (the
  terminal's echo is off for that line, and put back on Ctrl+C too).
- **Workspaces from Discord.** `/workspace folder:<path> name:<name>` makes
  one in a folder of the server, with its orchestrator, and its channel
  follows.

The desktop runs the same connector for its own workspaces
(discord-connector.md, "Where it runs").

Still to do: the remote workspaces a desktop drives on a node moving to the
node (the desktop then their client), and a phone reaching a node with no
desktop.

## What it costs, measured

`cargo bench -p hangar-engine --bench host` (benches/host.rs) drives the
host where it is busiest, with shells that only record: the panes' screens
fed at once, the output events of the reader threads, the mod's reports
taken in by a host of 8 workspaces of 6 panes, model writes, saves. Measured
on November 12, 2026, on a 4-core Xeon at 2.1 GHz (a shared cloud machine: one
run of each, figures move by about 10 % from a run to the next), before and
after the changes it led to:

| | Before | After | What changed |
|---|---|---|---|
| 4 panes printing at once, screens fed | 67 MB/s together | 279 MB/s | A lock per pane's screen (`Screens`, `Fed`) instead of one for all: a pane that prints holds up nobody else's, and its reader thread looks nothing up. |
| 16 panes printing at once | 63 MB/s | 244 MB/s | The same; one pane alone is 75 to 87 MB/s either way. |
| A chunk printed, beside its screen | 382 ns, 1,240 ms of processor for a million | 32 ns, 90 ms | Noted in two atomics of the pane's own (`Printed`), handed to its machine as one burst on the host's tick (250 ms, the desktop's), instead of two locks of the whole host per chunk. |
| A report of the mod, taken in | 14.8 µs | 6.5 µs | The snapshot is published only when what it holds changed (a tool call moves nothing the engines read), and the panes are reconciled with the state only when the state, or which workspaces are up, did; no workspace is copied per pane to find that nothing is to start. |
| The same, everything it sets off | 68.5 µs of processor | 10 µs | The orchestration runtime, the app's too, takes what waits in one trip to the blocking pool rather than one per report (`MAX_BATCH`). |
| A model write applied and published | 104 µs | 85 µs | The lighter reconcile. |
| The state on disk (48 panes) | 17,104 bytes, pretty | 9,981 bytes | Written compact and the owner's alone (0600), as the desktop writes its own; a save takes 0.6 ms either way, and runs 400 ms after the last change. |

What the figures do not show: a real Claude Code's output (the sample is
generated, after hangar-term's `claude`), real PTYs (`--bench feed --pty`
of hangar-term measures the reader thread), and a host under memory
pressure.

### At scale

`cargo bench -p hangar-engine --bench scale` (benches/scale.rs), same
machine, same day:

| | Measured | |
|---|---|---|
| A pane's screen, its history full (120 columns) | 3.0 MiB at 1,000 rows, 5.8 MiB at 2,000, 14.2 MiB at 5,000 | A row of cells is 2.9 kB, paid in every pane. The host kept 5,000 rows; it keeps 2,000 (`screen::HISTORY`): the longest read an agent may ask for is 1,000 lines, and nobody scrolls a screen with no window. |
| The state: 8, 50, 200 workspaces of 6 panes | a write 39 µs, 239 µs, 1.0 ms | Linear: the panes' index is rebuilt at each write. Writes come at an agent's pace, not a report's. |
| The same, saved | 0.9 ms, 2.2 ms, 5.4 ms | Flushed to disk before it takes the state's name, and its folder after (a power cut would otherwise leave an empty state, which a node refuses to start on). Off the runtime's threads, 400 ms after the last change. |
| The same, loaded | 0.65 ms, 3.7 ms, 20 ms | Once, at start. |
| A transcript of 1, 10, 50 MB: its newest page | 2.8 to 2.9 ms the first time, 57 µs again | Read from its end, whatever its size. |
| A board of 100, 500, 2,000 tasks, read | 0.03, 0.13, 0.58 ms | |
| A host of 48 panes where nothing happens | 1.0 ms of processor a second | The tick, 250 ms. |

### The node, from outside

`python3 scripts/node-bench/bench.py target/release/hangar-node` starts the
release binary in a home of its own and asks it over its API as a desktop
would (standard library only; `start`, `memory`, `mcp`, `flood` pick one):

| | Measured |
|---|---|
| 8 workspaces of 6 panes, from `serve` | the API answers after 49 ms, every shell runs after 0.11 s, every agent's command is typed after 0.67 s (0.5 s of it is the wait for each shell to fall quiet) |
| The daemon with those 48 agents started | 33.6 MiB (0.70 MiB a pane) |
| 16 panes, once each printed 12,000 lines | 137.8 MiB, 8.6 MiB a pane: the screen (5.8) and the byte history a desktop replays (4 MiB at most). It was 272 MiB, 17.0 a pane, with 5,000 rows of history. |
| An agent's `workspace_list` through Hangar's MCP server, answered by the node | median 0.31 ms, 95th of a hundred 0.48 ms |
| Its `pane_list` | median 0.53 ms, 95th 0.78 ms |
| 200 MB printed by a pane the node runs itself | 70 MB/s, 13 ms of the daemon's processor a MB (the screen is fed) |
| The same by a pane it runs for a desktop | 83 MB/s, 5.3 ms a MB (no screen: the desktop draws it) |

That bench is also what found that an agent in no pane was told to "open
the desktop" on a node with workspaces of its own and no desktop (above,
"Authority").
