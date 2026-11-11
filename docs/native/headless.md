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
   2. what the panes are doing (`panes_logic`), its clock on tokio rather
      than iced subscriptions;
   3. the agents' operations (`control`) and the model writes they ask for,
      applied by the engine to its own state;
   4. the orchestration engines (`orchestration`);
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
