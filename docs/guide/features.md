# Features

[← Hangar](../../README.md) · [Guide](README.md)

![HangarAI 2.0 on the checkout-v2 workspace: Iris, the orchestrator, sums up the round her workers just finished. Leo's partial refunds and Zoe's CSV export passed their check and their review, all 95 tests pass, and two deliveries wait for you to merge them; she lists what is left for you, the EU card provider and the shipping rates among it. Leo and Zoe, each in a git worktree of its own, have handed in their branches. On the right, the inspector shows Iris's session, her context use, her cost and the Claude plan's usage.](../../.github/assets/hero.webp)

<p align="center"><sub>Hangar 2.0's panes view on checkout-v2, a sample repository.</sub></p>

HangarAI is a native desktop app for Windows, macOS and Linux. It runs coding agents,
Claude Code and Codex (beta), as many sessions as you need, each in a real terminal next to
plain shells, and gives them a shared place to coordinate.

**Hangar 2.0 is native.** The whole app is written in Rust and drawn with
[iced](https://iced.rs), with no web view in it: it starts faster, and sixteen busy
terminals take a few hundred megabytes instead of several gigabytes
([Performance](#performance)). It reads the data of Hangar 1.x as it is, and a 1.x install
updates to it by itself.

HangarAI launches the agents installed on your machine, Claude Code, Codex or both, with your
accounts and your settings. Either one alone is enough. It is not a model provider: your
agents run on your own subscriptions. Its hosted features, [HangarAI Cloud](#hangarai-cloud),
are paid with HangarAI credits and cost nothing until you use them.

## An interface built like an IDE

- **Navigator** on the left: workspaces and folders, and each workspace's agents and their
  state at a glance.
- **One view of tiles** for each workspace: the conversation with its orchestrator, its
  terminals, its agents, what waits for you, its cloud runs, its board, kept in layouts you
  switch between with the tabs of the bar. See [Tiles and layouts](#tiles-and-layouts).
- **Command palette** (`Ctrl`/`Cmd`+`Shift`+`P`). Every command, pane and workspace is
  behind one search field.
- **33 themes.** Graphite is the default; the others are ported from popular VS Code
  palettes. Each theme restyles the whole window, not only the terminals.
- **See-through window** (Linux). **Settings → Appearance → Window opacity** lets the desktop
  show through the window (50 to 100 %); text, menus and dialogs stay sharp. The blur is up to
  your desktop: KDE does it, GNOME with an extension such as Blur my Shell. On Windows and
  macOS the window stays opaque for now.
- **Keyboard and language.** Every shortcut can be rebound, and `F6` moves between
  regions. The interface is in English or French.

## Agents and terminals

- **Claude Code, Codex and shells.** Each pane runs Claude Code, Codex (beta) or a plain
  shell: PowerShell, cmd, Git Bash, WSL, zsh, bash, fish, Nushell… A pane saved with Gemini
  CLI or OpenCode, which 2.0 dropped, opens as a shell in the same folder. What differs with
  Codex: [Codex (beta)](codex.md).
- **Terminals of its own.** Hangar draws its terminals itself, on alacritty_terminal: true
  colour, mouse modes, links, search, bracketed paste, and dead keys that compose.
- **Clickable file paths.** A path a pane prints (`src/a.rs:12:5`, `a.ts(12,5)`, a Python
  traceback's `", line 12`, a `git status` line) is underlined under the pointer when the file
  is there: a click opens it in the workspace's preview at that line, marked; a folder opens in
  the file manager; Alt-click (Option on macOS), or *Open with VS Code* in its right-click menu,
  opens it in VS Code at the line.
- **Session resume.** Agent panes reopen on the conversation they were in.
- **Runs in the background, once.** Closing the window leaves HangarAI running: agents keep
  coding, deliveries land and Hangar Mobile stays connected. The dock, the tray icon (*Show
  HangarAI*, how many agents work or wait for you, *Quit HangarAI*) or opening HangarAI again
  brings the window back, at its size and place: a second launch never starts a second copy.
  *Quit HangarAI* (`Ctrl`+`Shift`+`Q`, `Cmd`+`Q` on macOS) asks first while agents are
  working. Turn the background off in **Settings → General**.
- **Several accounts.** Claude Code and Codex accounts sit side by side (**Settings →
  Accounts**), chosen per pane, workspace or folder. With more than one in use, the status
  bar shows a chip for each, with how many panes run on it; click one to see where its
  agents are and to switch. **Switch accounts before a limit** (off by default) starts a new
  pane or worker on the account with the most room once its own has passed 80, 90 or 95 % of
  its window, and an agent stopped at its limit gets a card in Needs you to restart on
  another account, on the same conversation.
- **A side channel per agent.** For Claude Code, a small mod (hangar-bridge); for Codex,
  Hangar attaches to the server Codex's own terminal runs on. Either reports turns,
  permission prompts, context fill and plan usage, so pane states are exact rather than
  guessed. When an agent edits a file another pane just changed, it is told who changed it
  and asked to re-read it. Without one (the mod off, a Codex Hangar cannot attach to), Hangar
  reads the terminal as before.
- **Broadcast.** Send one instruction to every pane at once. Agent panes with a side channel
  can also hold it until their agent is free.
- **Pane tools.** Zoom a pane to the full grid, search its scrollback, drop a file on it
  to paste its path, or run a saved command in it.
- **Pinned panes.** Pin a pane (*Pin in place* in its « … » menu, or the pin of a preview's
  header) and it stays exactly where it is: nothing moves as you pin it, and from then on
  it keeps its place and its size however many panes and workers open, close or move. The
  others share the rest of the grid and a new pane never lands in its space; several
  pinned panes each keep their own place, after a restart too. When too few panes are left
  to fill the room around a pinned pane, what they cannot fill stays empty until the next
  pane opens there; the last pane left has the whole grid. A pin shows beside its name.
  Drag one of its borders to resize it: it is held at its new size. Click the pin, or
  *Unpin this pane*, to let it go, which moves nothing either; a pinned pane is not moved
  by drag and drop until then. Its size follows the window when the window itself is
  resized, like every pane. *Pin the focused pane in place, or unpin it* is in the palette,
  and can be given a shortcut in the settings.

## Workspaces

- **Saved workspaces.** A workspace is a project folder and its layout, and reopening it
  brings every agent back. Workspaces can be grouped in folders, or moved to a window of
  their own.
- **From a Git repository.** In the new-workspace dialog, choose *From a Git repository*,
  paste `owner/repo` or an https or ssh address, and see the folder it will be cloned into
  before anything runs. Hangar clones it (with progress, and a Cancel), opens a workspace
  and starts an orchestrator that reads the repository and gets it ready. A folder that
  already holds the repository is reused. Set the folder repositories go in under *Projects
  folder* in the settings; left empty, it is next to the current workspace's folder.
- **40 panes.** A workspace holds 40 panes, terminals and views together, and up to 100 in
  its orchestration settings; past 16, a new pane also needs 1.5 GB of free memory and
  processors that are not saturated.
- **Sleep.** A workspace idle for 10 minutes is put to sleep to free memory. When you open
  it again, its agents resume their sessions. You can change the delay or turn this off.
- **MCP servers per workspace.** Choose which MCP servers a workspace's agent panes start,
  instead of starting every configured server in every pane.
- **Git worktrees and diffs.** Create, lock and remove worktrees, open one in a pane or in
  its own workspace, and see what an agent changed in its folder.

## Board, memory and statistics

- **Task board.** Priorities, dependencies, labels, assignees and comments. It is a tile:
  compact (the tasks that matter now) or whole once wide enough. The bar's Board icon
  (`Ctrl`/`Cmd`+`Alt`+`B`) opens it over the grid, `Esc` goes back; `Ctrl`/`Cmd`+`Alt`+`Shift`+`B`
  pins one along the bottom of the layout. Stored in `.hangar/board.json` in the project.
- **Global memory.** Agents search, write and update facts across projects. You can browse
  and edit them in the Memory tile (`Ctrl`/`Cmd`+`Alt`+`K` over the grid).
- **Usage statistics.** Tokens, active time and the equivalent cost at API prices, per
  project, model and agent. They are read from the transcripts Claude Code and Codex keep on
  disk; the sessions of a project's worktrees count under the project, even once the
  worktree is gone. On a subscription, the cost figure is an equivalent, not a bill. In the
  Statistics tile, `Ctrl`/`Cmd`+`Alt`+`U` over the grid.
- **Replays.** Show what your agents built: **Export replay…**, in a workspace's menu,
  turns a period of its board into a page. See [Replays](#replays).

![The board of a workspace: one task to do, blocked until you pick the EU card provider; two in review, each labelled ready-to-land, with the worker that did it; and seven done](../../.github/assets/board.webp)

![The usage statistics view over 30 days, filtered on one project: tokens, API-equivalent cost, active time and sessions](../../.github/assets/stats.webp)

## HangarAI Cloud

Hosted features, paid with HangarAI credits: Pro includes 3,000 a month, Max 10,000 and the
trial 150 ([the plans](install.md#the-trial-then-a-plan)). Nothing is spent until you use one,
and each shows what it costs, in credits with its euros beside it, before it runs.

- **The Cloud space.** The cloud chip in the status bar, or **Show HangarAI Cloud** in the
  command palette: your balance, the machines and share links at work, and the history of
  what each service spent, by day, filtered by service and period, with a receipt for every
  line. **Settings → Cloud** lists each service with its rate and what leaves this computer;
  packs of credits and pay as you go are in **Settings → License**.
- **Dictation.** Hold a key, speak, let go, and the text is typed into the focused pane. The
  recording goes to HangarAI's server, which has Groq transcribe it on HangarAI's own key;
  neither keeps the audio or the text. About 0.66 credit a minute (Settings → Dictation shows
  the rate); a dictation holds two minutes at most, silence is never sent, and one that
  failed on the way can be tried again.
- **Image generation.** Ask an agent for an image: it draws or edits one with its
  `image_generate` and `image_edit` tools, in *Draft*, *Standard* or *Best*. The image opens in
  a preview pane while it is drawn and is saved in the workspace (`.hangar/images/`, never
  over a file already there), and the agent's answer gives its path and what it cost. An image
  the model refuses or the safety filter holds back costs nothing. The conversation shows it
  as a card. The prompt, and the image to edit, go to fal, which is told not to keep them.
- **Share links.** **Share link**, in a preview pane's header, publishes what it shows (an
  image, a page with its pictures, a PDF, a Markdown document, a 3D model), up to 100 MB, for
  30 days. The sheet says what goes, until when and what it costs, then copies the link and
  shows its QR code. Anyone with the link opens it at hangarai.dev/s/…, served from
  share.hangarai.dev, a domain apart from everything else, with a *Report* button. The pane's
  *Shared* chip and the Cloud space extend a link (up to a year), copy it or delete it. A page
  with a password field, a card form or a form that posts is refused, and a trial keeps 3
  links live at once. Agents publish with `share_publish`, after your OK unless the
  workspace lets them, and list and delete the links they published.
- **Cloud Builds.** A build, a test suite, a training run or a delivery's check on a machine
  rented for you alone, billed by the second: 32 or 64 CPU threads, or a GPU (an RTX 4090, an
  A100 or an H100, or several), or the exact machine you describe, with a time limit from 15
  minutes to 24 hours. **Cloud: Run in the cloud…** in the command palette, `hangar-ai cloud run
  -- <command>` in a terminal, an agent's `cloud_run`, or **Run delivery checks on → HangarAI
  Cloud** for a workspace's deliveries ([Orchestration](orchestration.md)). For Pro and Max;
  [Cloud Builds](cloud.md) has the details.
- **Hosted nodes.** **Keep running in the cloud…**, in a workspace's menu: a server rented
  for you alone, in Germany or Finland, by the started hour; your workspaces move there and
  keep working while your computer is off, then come back, sealed to this computer, when the
  session ends. For Pro and Max; [Hosted nodes](hosted-nodes.md) has the details.
- **Hangar Mobile from anywhere**, without Tailscale, through HangarAI's relay, encrypted end
  to end between your PC and your phone ([Hangar Mobile](mobile.md#from-anywhere-without-tailscale)).
- **Agents' spending.** Each workspace gives its agents a daily budget of credits (100 by
  default). Past it, and before any share link, an agent asks you in **Needs you**: *Allow
  once*, *Allow 200 more today* or *Deny*; it is told your answer and carries on.

## Replays

**Export replay…**, in a workspace's menu in the navigator, turns what its agents did over a
period (the last session, 24 hours, 7 days or everything) into a page to share: the board
moving as tasks start and land, each delivery with its summary, the agents and the time it
took, with a timeline to scrub, in light and dark, on a phone too.

- **You choose what goes.** Untick the tasks to leave out, and choose whether the delivery
  summaries, the progress notes and the pictures the deliveries added to the repository go.
- **What must not leave stays.** Secret-looking strings, e-mail addresses, share links, paths
  outside the repository and names are taken out, and the sheet says how many. No file's
  contents, task description or agent's own comment ever goes: of the agents, only their
  names.
- **Preview** opens the page on this computer first.

The page is published as a [share link](#hangarai-cloud), for 30 days, and carries your
referral code (from your HangarAI account, or typed in): its *Try it* leads to your invite
link.

## Kits and previews

- **Kits** bring a domain into Hangar: a plugin for the agents (skills, MCP servers,
  hooks…), setup commands, and tabs of their own. A tab is either an HTML page or, new in 2.0, a native tab written in Rust and
  compiled to WebAssembly, which Hangar draws in your theme. The **Library** in
  **Settings → Kits** lists the public kits of the [hangar-ai](https://github.com/hangar-ai)
  organization. See [Kits](#kits).
- **Previews.** Agents show you files as panes of the grid, beside their terminal: images
  (animated GIF and WebP too), code and text with highlighting, Markdown, PDFs, web pages,
  video and audio. A file can come from any folder of your computer, not only the
  workspace's; the viewers are only ever served the files that were shown. **Share link**,
  in a preview's header, publishes it ([HangarAI Cloud](#hangarai-cloud)).
- **3D preview.** Agents show you a model (glTF/GLB, OBJ, STL, PLY, FBX) from any folder
  in a pane of the grid, its textures loaded from beside it, with its glTF animations, and
  render snapshots to check their own work.
- **Your own control layout.** A preview and a kit's tab are panes like the terminals: drag
  them by their header onto another pane, resize them with the dividers, zoom them, reach
  them with `Ctrl`+`1`…`9`. Kit tabs come back where you left them after a restart.

What needs a browser engine, a PDF, a web page, a video or an HTML kit tab, opens in
`hangar-webview`, a small helper program beside Hangar, in a window of its own; its pane
holds a card that brings the window forward, and closing the pane closes the window.

A web page an agent shows runs isolated: its scripts run, but it reaches neither Hangar,
your files, nor the network — nothing it fetches, posts or loads from a server, the
servers of your own computer included — only its inline scripts and styles and the
pictures, video and sound beside it. If a page needs the internet (a chart library from a
CDN, say), the network button of its bar lets that page alone reach it, over HTTPS only;
the servers of your computer and your local network stay out of its reach.

## Tiles and layouts

A workspace is one view: a grid of **tiles**, kept in named **layouts** whose tabs are in the
bar. A tile is a pane of the grid: it moves by its header, resizes by its edges, zooms and
pins as any pane, and `Alt` with the arrows goes from one to the next.

- **The tiles.** The conversation with the orchestrator, a terminal, Agents (who does what),
  Needs you (what waits for you, answered in place), Cloud and a cloud run, Machines, Board,
  Activity, Changed files, Deliveries, Tonight, Shared, Accounts & credits, Memory,
  Statistics, and the previews (a file, a 3D model, a kit's tab). **+ Tile** in the bar
  (`Ctrl`/`Cmd`+`Shift`+`T`) adds one beside the focused tile, or where you drop it.
- **Two layouts come with every workspace.** **Chat + team**, the default: the
  conversation pinned on the left, Agents beside it, and Needs you, Cloud and Machines
  arriving by themselves the first time there is something to show; every terminal is
  hidden there, its agent still at work. **Terminals**: every terminal in a grid, the
  orchestrator on the left. There is no layout for the cloud: any work can start a cloud
  run, so when a run starts — yours, an agent's, a delivery check — the **Cloud** tile
  arrives in the layout on screen and in your other layouts (**Machines** too when a machine
  shows up), never in **Terminals** nor where you closed it. A workspace that had the
  **Cloud** layout of the first 3.0 builds opens without it, or keeps it as yours if you
  changed or renamed it. Make your own with the **+** after the tabs, rename one, reset a provided one; `Ctrl`/`Cmd`+`K`
  then a digit goes to the n-th, `Ctrl`/`Cmd`+`Alt`+`L` opens the switcher, and
  `Ctrl`/`Cmd`+`Alt`+`T` opens **Terminals**. Switching restarts nothing.
- **Hiding a terminal** (its ×, or `Ctrl`/`Cmd`+`Shift`+`X`) takes it out of the layout while its
  agent keeps working; stopping it is in its menu, named. The status bar counts the tiles
  and the terminals the layout hides, « 8 tiles · 2 terminals hidden »: a click lists them
  to show one again.
- **Arrange** in the bar shows every handle, every boundary's size and every free place,
  where a **+** adds a tile; `Esc` or *Done* ends it.
- **Board, Memory and Statistics** are the three icons at the right of the bar: each opens
  its tile over the grid without touching the layout, and `Esc` goes back. *Keep in this
  layout*, in the bar over it, keeps the tile there.
- **A small window** stacks the tiles beside the main one, then shows them as tabs, and
  gives the layout back as it was once it grows again.

A workspace that 2.x showed as the chat view opens on **Chat + team**, its grid kept as
**Terminals**; the inspector's session and context are in the headers, its files in Changed
files, its activity in Activity, and `Ctrl`/`Cmd`+`Alt`+`I` opens Agents.

The conversation tile:

- The agent's questions and permission prompts become cards you answer in the
  conversation, and the images a Codex agent makes show in its messages; kits, previews and
  the 3D view open as tiles beside it.
- **Drop anything on the conversation**: a folder, a file, an image, several at once, from any
  folder. Each becomes a chip in the composer, where the caret was (its icon, its name, its
  whole path on hover; Backspace or its × takes it out), and goes in the message as its
  absolute path, the way the agent opens it, a folder with its trailing separator. While a
  question waits, the path goes into its answer field. In your messages, a path that exists
  is a link: a file opens in the preview at its line, a folder in the file manager.

## Nodes

Run a headless HangarAI, `hangar-node`, on a Linux machine you reach over SSH (x86_64 or
arm64: a server, a VPS, a GPU box, a Raspberry Pi). Add it in **Settings › Nodes** with its
SSH target: HangarAI installs it with your SSH keys, starts it and keeps a tunnel to it,
with nothing to open on the server. A workspace can then live on the node: its terminals,
agents, board and memory run there, the window shows them like a local workspace's, and it
catches up on what happened while your computer was off. The navigator and the palette show
which machine each workspace runs on, and a worker can be sent to a node for a heavy build.

No server of your own? **Keep running in the cloud…**, in a workspace's menu, rents one for you
by the started hour, and the node's card installs and signs in Claude Code and Codex on any
node: see [Hosted nodes](hosted-nodes.md).

## HangarAI on Discord

Your workspaces on a Discord server of your own, as on the phone: a channel per workspace
(those on your nodes included) with what its orchestrator does and says, cards to allow,
answer, approve and land, and a thread per worker. Set it up in **Settings › Discord**: the
token of a bot you create at discord.com/developers and your server's ID, then invite the
bot and send it the pairing code it gives. Only the account you pair is heard, and a server
of your own is best: what is written there reaches your agents. The bot runs while HangarAI
runs on this computer, window closed or not; once you are paired, your nodes answer in
their own channels with the computer off.

**Settings › Discord** also has **Discord Rich Presence**, off unless you turn it on.

## Pane scopes (Linux)

Each pane runs in a systemd scope of its own, named after it, so when memory runs out the
system stops the pane that uses it, a build for instance, rather than HangarAI and every
agent with it. On by default where systemd's user manager runs; elsewhere the panes run as
before, without a word. **Settings › General › Memory › One systemd scope per pane** turns it
off for the panes started afterwards.

## Kits

A kit links a domain, a game engine, a studio's tools, a service, to a workspace, without
Hangar knowing anything about it. It can bring a plugin for the agents (skills, commands,
agents, MCP servers, hooks), setup commands that you see verbatim and run once, and tabs:
pages of its own that talk to Hangar through a small, declared API, each permission accepted
by you. Claude Code loads the whole plugin; Codex loads its MCP servers at each launch, and
its skills and hooks once you **Add to Codex** the kit; each kit lists what reaches which
agent.

- **HTML tabs** are the tabs of 1.x, with the same `window.hangar` API, the same
  permissions, and Hangar UI, the stylesheet that makes them look like Hangar. In 2.0 they
  open in the `hangar-webview` helper, in a window of their own.
- **Native tabs** (new in 2.0) are Rust compiled to WebAssembly with the kit SDK,
  `hangar-kit-sdk`. A tab describes its page as widgets, and Hangar draws them with its own,
  in your theme and language. The module runs in a sandbox with time and memory limits, and
  Hangar answers only the calls its manifest declares. A kit can carry both kinds, so that it
  still opens on 1.x.

Add a kit from a folder, a GitHub link or the Library. To write one, start from
[`kit-template`](https://github.com/hangar-ai/kit-template).

## Performance

Hangar 1.12.1, the last release built on a web view, against Hangar 2.0, on the same Linux
machine:

| | 1.12.1 | 2.0 |
|---|---|---|
| Start, cold | 0.96 s | 0.57 s |
| Memory at rest | 238 MiB | 166 MiB |
| 16 terminals printing non-stop for 30 s: memory | 8.1 GiB | 249 MiB |
| 16 terminals printing non-stop for 30 s: processor | 4.4 cores | 2.4 cores |
| Echo of text sent to a pane while 15 others print | no echo within a minute | 2.5 ms |

Each terminal parses its output on a thread of its own, and the window draws at most once
per frame whatever they print, so a busy pane never holds up the one you type in. The method
and the raw figures are in [docs/native/BENCHMARK.md](../native/BENCHMARK.md).
