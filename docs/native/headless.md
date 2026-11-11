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
   answered there. A desktop that connects shows and drives them as a
   client. Existing remote workspaces move to the node.
4. **The Discord connector in the engine** (discord-connector.md): it runs
   where the engine runs, on the node for its workspaces with the PC off.
5. **A node that starts on its own**: its settings (the bot's token among
   them) on the server, `hangar-node serve` with no desktop, workspaces
   created from Discord.

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
