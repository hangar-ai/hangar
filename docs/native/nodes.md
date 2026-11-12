# Hangar nodes (experimental)

A *node* is a headless Hangar running on another Linux machine (a server, a VPS,
a GPU box, a Raspberry Pi), driven from the desktop Hangar over SSH. Everything
here sits behind the `nodes` experiment.

The person's decisions: first, whole workspaces that live on a node; then,
workers sent from a local workspace to a node, on the same base. The desktop
reaches a node over SSH (its keys, nothing to open on the server). Nodes run on
Linux x86_64 and arm64.

## What runs where

| | Desktop | Node |
|---|---|---|
| Window, panes' terminals (`hangar-term`), chat view | yes | no |
| PTYs: shells, Claude Code, builds | local workspaces | workspaces on the node |
| Board, memory, MCP server, deliveries | local workspaces | workspaces on the node |
| Transcripts (`~/.claude`) | local | on the node |

A workspace on a node is drawn by the desktop exactly like a local one. Its
terminals are fed by a byte stream from the node instead of a local PTY; its
board and memory are read from the node's API instead of the local disk. Close
the laptop: the node keeps running, and the desktop catches up from the PTY
history when it reconnects.

## Pieces

1. **`hangar-core`, headless.** The desktop-only parts of core (voice with
   whisper/cpal, Discord, notifications, autostart, the phone server) move
   behind a default `desktop` feature. `headless::start` runs what a node
   needs: the PTY manager, the board and memory stores, the HTTP API on
   loopback with its tokens, the bridge and deliveries.
2. **`hangar-node`** (new crate, binary `hangar-node`). `serve` starts the
   headless core plus the *node API* on 127.0.0.1 and writes
   `~/.local/state/hangar-node/node.json` (port, token, version, pid); `--mcp`
   is the MCP stdio proxy agents on the node run, as `hangar-ai --mcp` does on
   the desktop; `version` prints the version and target. Linux only.
3. **Node API** (HTTP + WebSocket, Bearer token, loopback only, reached through
   the SSH tunnel):
   - `GET /node/info`: version, target, hostname, home, whether `claude` and
     `git` are on PATH;
   - `POST /node/panes` spawn (id, cwd, shell, size, agent, env bits),
     `POST /node/panes/{id}/input`, `/resize`, `DELETE /node/panes/{id}`,
     `GET /node/panes` (live panes);
   - `GET /node/panes/{id}/stream?from=<seq>` (WebSocket): the history after
     `seq`, then live output frames `{seq, data}` and `{exit}`;
   - `GET /node/events` (WebSocket): the node's core events (board changed,
     memory changed, bridge events) forwarded as JSON, and `own:changed`
     when the state of the workspaces the node runs itself moved;
   - `GET /node/fs?path=` list a folder (to pick a workspace's folder);
   - `POST /node/clone` `{url, parent, name, jobId}`: a repository cloned on
     the node (core's `git_clone`, its progress on `/node/events` as
     `git-clone:progress`), `POST /node/clone/cancel` `{jobId}`;
   - the existing `/api/board`, `/api/tasks/...`, `/api/memory/...` routes of
     core's server, with the App token.
4. **Desktop: connection** (`hangar-app/src/nodes/`). A node is
   `{id, label, ssh target, kind}` in the settings (the kind, a server,
   a laptop…, picks the icon it goes by). Connecting runs, with
   `ssh -o BatchMode=yes`: `uname -m` → picks the binary for the target;
   installs or updates `hangar-node` under `~/.local/share/hangar-node/bin`
   when the version differs (the binary comes from beside `hangar-ai`, or
   `HANGAR_NODE_BINARY` for development); starts `hangar-node serve` detached
   (a `systemd-run --user` unit when available, else `setsid nohup`); reads
   `node.json`; opens `ssh -N -L 127.0.0.1:<local>:127.0.0.1:<port>` and keeps
   it up. A node that is switched on (`enabled`, on unless set to `false`) is
   connected from the app's start and reconnected for as long as it is down,
   whether or not it ever connected: 2 s doubling to 60 s between attempts,
   each wait spread by ±20 %, started over only once a connection held 30 s,
   and cut short when the window gets the focus back or the person asks
   (« Retry now »). One that is switched off is not connected, and its
   workspaces are out of the navigator and the palette until it is switched
   on again. Status per node: connecting, connected, unreachable (with the
   reason). A link that drops says nothing for 5 s; how a machine and its
   link are then shown is decided in one place (`nodes/presence.rs`) for
   the navigator, the palette, the pane's bar and card and the settings.
   Why an attempt failed is one short phrase for each thing that can fail
   (`nodes/failure.rs`: « unreachable over SSH (Connection timed out) »);
   what the tool said in full is a second, quieter line in the settings
   and the tooltip of the pane's card.
5. **Desktop: remote workspaces.** `Workspace.node: Option<NodeId>`; its `cwd`
   is a path on the node. `PaneLink` and the pane controller talk to a
   `PtyBackend` (local: today's `PtyManager`; remote: the node API), so the
   terminal, the hub and the frame logic stay as they are. The board and
   memory views read and write through the node API for such a workspace and
   refresh on the node's forwarded events.
6. **Test rig.** A Docker image (Debian, openssh-server, git, a test user with
   a key made for the tests) and an end-to-end test: build `hangar-node`,
   start the container, connect over SSH, start the node, open the tunnel,
   spawn a shell, type `echo`, read it back, kill it, reconnect and replay
   the history.

## End-to-end test

`bash scripts/node-test/e2e.sh` runs the whole flow on the Docker rig against
an isolated debug instance: remote shell, reconnect replay, memory and
transcript routes, orchestration in a remote workspace, and a local
workspace's worker sent to the node whose commit lands back here. It needs
Docker and a display and takes minutes, so it is not in CI: run it by hand
after a change to nodes. See `scripts/node-test/README.md`.

## Workspaces of the node's own

A node also runs workspaces itself, with no desktop: hangar-engine's host
in the daemon, their state on the node, their agents' requests answered
there (headless.md, step 3). They are made from the server (`hangar-node
workspace add <folder>`), the node API (`/node/own`) or Discord
(discord-connector.md).

A desktop that has the node among its machines mirrors them: each is a
workspace of its own state marked `hosted`, listed in the navigator with
the machine's other workspaces and opened like them. Their state is read
when the node connects, when it says `own:changed` and when events were
lost; an answer older than what is held (same `boot`, lower `revision`)
changes nothing. While the node is away the mirrors stay as last seen, and
a node removed from Settings takes them with it. Settings › Nodes says how
many a machine runs.

The node stays their owner. The desktop follows their panes' streams and
never starts, launches or kills one; its orchestration runtime, its
hibernation, its Discord connector and the answers it gives a node's
agents leave them out; the reports of their agents and their queues come
over `/node/events`, so their marks show for a workspace never opened.
Where a mirror is listed, its layout, its views, its window and its chat
view are the desktop's own, and change freely.

What is the node's of a workspace (its name, its folder, its panes, its
settings) is changed on the mirror and told to the node. `Model::apply`
applies the person's change at once, so the pane just opened or restarted
has its id here, and leaves the ops that say it (`hosted::diff`) in
`nodes::hosted::Outbox`; after every message the shell sends what waits,
one request at a time for each node (`POST /node/own/workspaces/{id}/ops`),
so a node hears the changes in the order they were made. A pane opened or
restarted on a mirror keeps the id the desktop gave it. Until the node has
answered everything sent to it, what it lists is older than the mirror and
is not taken in; then it is read again, and its word is the last. A change
the node refuses, or that did not reach it, ends the line there: the
mirror is read again from the node and one notice says what was not done.
While the node is away or switched off such a change is refused with a
notice, nothing applied. Only the person's changes are sent: what an agent
of this computer asks through the control port is refused
(`Model::apply_unattended`).

Deleting a mirror deletes the workspace on its node, after a question that
says so; its folder stays there, and nothing of this computer is cleaned
up after it. Pinning an orchestrator on a mirror restarts the pane with
the role, on its node and on the same conversation (a pane this computer
runs waits for its turn to end; the node's is restarted at once). « Put to
sleep » does not apply to a mirror.

A workspace made from the new-workspace dialog with « Runs on » set to a
node is the node's own from the start: built here, with the ids the
desktop gives, flagged `hosted` and handed to the node (`POST
/node/own/adopt`) through the same line; were the node to refuse it, it is
taken back out and the reason shown. « Runs on » is there whatever the
workspace is made from, with this computer and every node; a node that is
switched off or cannot be reached keeps its place, greyed, its state beside
its name. A clone from Git with « Runs on » set to a node is made by the
node (`POST /node/clone`), with the node's own access to the repository,
into the folder typed or picked among the node's (`projects` in its home
when none is); its progress comes over `/node/events` and is shown as a
local clone's, and when git fails its words are shown, with a line saying
that the node may lack access. The workspace made on the folder is then
handed to the node like any other. A workspace the desktop still drives on a node
(`node` set, not `hosted`: one made before, which stops with this computer)
has « Move to {machine} » in its menu in the navigator. After a question
that says what happens, it is flagged `hosted` here first, so that this
computer's engines let go of it, then handed to the node, which takes down
the shells the desktop had started and starts its own under the same ids,
each Claude Code on its session. Nothing of the workspace is followed here
until the node said yes; if it says no, the workspace is driven from here
again and its panes are found where they were. The move is refused, with
the reason, while one of its deliveries runs, while a worker has no
session yet, or while the node is away. Settings › Nodes says, under a
machine, how many workspaces there still need this computer on.

The state is not saved while a workspace is on its way to a node, and one
still on its way when the app closes is saved as this computer's: written
as the node's before the node took it, it would be a mirror of nothing and
go at the next sync. A node that did take it lists it at the next
connection, which makes it its mirror then.

The node API also lets a desktop mirror them, the node staying their owner:
`GET /node/own/state` (the workspaces whole, with a revision),
`own:changed` on `/node/events`, `POST /node/own/workspaces/{id}/ops` (what
the person changed on the desktop's copy) and `POST /node/own/adopt` (a
workspace the desktop ran on the node, or has just made, handed to the
node). `POST /node/panes` and `DELETE /node/panes/{id}` answer 409 for a
pane of the node's own. The shapes are in headless.md, step 3.

## One repository on several machines

A workspace's repository is the address of its `origin`, brought to one
spelling (`nodes/repos.rs`, `identity`): the host in lower case and the
path, without scheme, user, port, `.git` or a trailing slash, so
`git@github.com:Acme/Shop.git` and `https://github.com/acme/shop` are one.
An address with no host (a folder, `file:`) is no identity, and a
workspace without an `origin` is never grouped.

The address is read where the folder is, off the UI thread, and held in
memory only (`nodes::Live::repos`): by git for a folder of this computer;
by the node for the workspaces it runs itself, beside their state
(`origins` in `GET /node/own/state`, by workspace, so it is never saved
with one); by `POST /node/git` `{op: "origin"}` for a workspace this
computer still drives there. Credentials in an address are taken out
before it leaves its machine. It is asked for after any message that left
a workspace without it (one just added, a folder that changed), and read
again when a node connects and when the window comes back to the front.
None of it is read without a node in the settings.

When the same repository is open on more than one machine (a machine that
is switched off does not count, and two checkouts on one machine are not
each other's): a row of this computer gets one faint mark whose tooltip
says where else (« Also on vps (api) »); a row that already wears its
machine's mark says it in that mark's tooltip; the workspace's menu starts
with « Also on », a line for each of the others under its machine's icon,
which goes there. No row moves. « Open on another machine… », in the same
menu, opens the new-workspace dialog on the repository, « Runs on » set
to the first machine, this computer then the connected nodes, that has no
checkout of it; it is greyed when every connected machine has one.

## Later (phase 2 and on)

- Orchestration on a node (workers in worktrees there), the chat view of a
  remote workspace, previews and kits of remote files.
- Workers sent from a local workspace to a node: the worker clones the branch
  on the node and delivers it back.
- arm64 builds published with releases; Tailscale as a second transport.
