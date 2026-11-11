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
   5. the desk (`mobile`'s calls, without the phone's server).
2. **A host for the engine without a window**: the store and the
   read-models in one place, saved to disk on tokio's clock, the core's
   events fed to it, the writes applied there. The app becomes one such host
   with a window around it.
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
