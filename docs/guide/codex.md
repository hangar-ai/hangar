# Codex (beta)

[← Hangar](../../README.md) · [Guide](README.md)

Hangar runs [Codex](https://learn.chatgpt.com/docs/codex/cli), OpenAI's coding agent, the way
it runs Claude Code: in a real terminal you can take over at any time, as an orchestrator, a
worker, the project manager or a pane of your own, with the board, the memory and every
Hangar tool. Either agent alone is enough. With both installed, they work together: an
orchestrator of either agent hands tasks to workers of either agent, and one agent can hand a
sub-task to the other.

> [!NOTE]
> Codex support is **beta**. Hangar was written against the protocol OpenAI publishes for
> Codex (the schema, source and docs of its app-server, at Codex 0.170.0), without a recorded
> Codex session, so what that protocol does not say may be wrong here. If something does not
> work in a Codex pane, please
> [report it](https://github.com/hangar-ai/hangar/issues/new?template=codex-bug.yml): the form
> asks for `codex --version`, whether the pane was attached, and Hangar's log.

## Install and sign in

When no agent is found, Hangar opens a **Set up your coding agents** sheet at start, with a
card per agent; with one agent installed, the sheet can still be opened from the
new-workspace dialog (**Set up Codex**) and closes like any dialog. **Install Codex** runs
OpenAI's official installer for your user alone, with no administrator password, and shows
what it prints:

- macOS and Linux: `curl -fsSL https://chatgpt.com/codex/install.sh | sh`
- Windows, in PowerShell: `irm https://chatgpt.com/codex/install.ps1 | iex`

`npm install -g @openai/codex` and `brew install --cask codex` work too: Hangar finds `codex`
on your `PATH` and in the installer's folders. On NixOS, the installer's binary starts as it
is. Hangar never updates Codex itself.

**Sign in** opens Codex in a pane, where it asks for your ChatGPT account or an OpenAI API
key the first time; the sheet notices when you are signed in. Hangar never sees the key.

## Attached, or the plain terminal

For each Codex pane, Hangar starts a Codex server of the pane's own (`codex app-server`) and
types `codex --remote` into the pane: you get Codex's own terminal, unchanged, and Hangar is a
second client of the same session. That is **attached**, and it is what the hangar-bridge mod
does for Claude Code: exact pane states, Hangar's messages delivered without typing (into an
idle session, or steering a running turn), approvals and questions held until you answer from
the chat view, your phone or Discord (the first answer wins, the terminal's included), live
context use and exact plan limits with their reset times, generated images, and the session's
id without searching for it.

**Settings › General › Attach Hangar to Codex** (under Sessions) is on by default. Off, or when
Hangar cannot attach, the pane runs Codex's plain terminal, and Hangar works from what it sees
and the files Codex writes. The pane's status says why. On the plain terminal:

- whether Codex is working is guessed from its output, its title and its notifications;
- Hangar's messages are typed into Codex when it is idle; a running turn is not reached, so
  they wait for its end;
- approvals and questions are answered by typing keys into Codex's dialog, checked on screen
  first, from the chat view, Mobile or Discord as before;
- the role still rides on Codex's command line, and the workspace's state comes with the
  first message;
- context use and plan limits are read from Codex's session files, not live;
- the session's id is found in Codex's session folder.

Hangar attaches to Codex 0.157.0 and later. It was built against 0.170.0, and reads newer
releases leniently; an older one, or a release listed as broken, runs on the plain terminal
with a notice naming the version needed.

## Orchestrator, workers and reviewer

- **New workspace.** *Starts with* offers Claude Code or Codex for the orchestrator: by
  default the one in **Settings › General › Default agents**, else Claude Code when both are
  installed and the one installed otherwise. **More options** chooses the workers' agent
  (the orchestrator's by default). A missing agent is greyed, with a link to set it up.
- **Existing panes.** Pin a Codex pane as orchestrator from its **…** menu, as a Claude Code
  one. The orchestrator can start workers of either agent (`worker_spawn` takes `agent`).
- **Reviewer.** The delivery reviewer is the agent set in **Default agents**; a Codex review
  runs `codex exec` in a read-only sandbox, with the same time limit as Claude Code's.
- **Models.** **Orchestration…** of a workspace has a Codex model for the orchestrator, the
  workers and the reviewer, each a Codex model id; left empty, Codex picks its own. The
  Claude Code models (Opus, Sonnet, Sonnet) stay Claude Code's.
- **Permissions.** The workspace's permission mode maps to Codex's own: *Accept edits* runs
  commands in Codex's workspace-write sandbox and asks before anything outside it (Codex has
  no "edits only"); *Auto* adds Codex's automatic reviewer of approvals, the one
  `--approve-for-me` turns on; *Bypass* turns approvals and the sandbox off.

Hangar's MCP server reaches every Codex pane on the command line of its server
(`-c mcp_servers.hangar…`), at each launch: there is nothing to register, and Hangar never
writes your `~/.codex/config.toml`.

## Accounts

**Settings › Accounts** lists each agent's accounts in a section of its own. Your main Codex
account is your own Codex folder (`$CODEX_HOME`, else `~/.codex`), which Hangar only reads.
Each account you add is a Codex folder of Hangar's that shares your main one's configuration,
sessions, history, generated images, plugins and skills, with a login of its own: Hangar
opens a pane running `codex login` on it. A session started on one account can be resumed
from another. Who each account is (its e-mail and plan, or "API key") is read on your
machine, never sent anywhere.

Choose a Codex account per workspace, folder or pane, as for Claude Code. A workspace keeps
one account per agent, so its orchestrator can run on one Claude plan and its Codex workers
on a ChatGPT plan.

## Images

Codex makes images (OpenAI's image generation is on by default in Codex). Hangar picks up
each image a Codex session saves and shows it in the chat view's message, in the pane's
inspector, on Hangar Mobile and on the turn's card in Discord; click it to open it in a
preview pane. The file stays where Codex saved it, under `$CODEX_HOME/generated_images/`.

A Claude Code orchestrator can ask Codex for images through `delegate` (below) and move the
files it gets back into the project.

## Delegate

`delegate` hands a sub-task that is not a board task to an agent with no terminal: "make the
five icons", "have the other agent review this function", "answer this question about the
repository". A Codex child runs as a thread of a Codex server of its own, a Claude Code child
as `claude -p`. It works in the caller's folder, or in a worktree and branch of its own made
from the caller's last commit. Its result (its final message, the files it changed, its
images, its branch) comes back to the caller as a message when it ends, and
`delegation_status`, `delegation_cancel` and `delegation_list` follow it.

A child runs under the workspace's permission mode, on the workspace's account. What it asks
you, a permission or a question, waits in **Needs you**, the chat view, Mobile and Discord:
only you answer it. A Codex child makes images only when the caller asks for them, so a
review cannot spend image quota.

## What each agent can do

Hangar reads what an agent can do, never which agent it is: each feature asks for a
capability, and degrades to the plain terminal's level when the side channel is missing.
This table is written by hand from the code: the `CLAUDE` and `CODEX` rows of `AgentInfo` in
`crates/hangar-core/src/agent.rs`, the levels of `crates/hangar-core/src/agent/caps.rs`, and,
for the last column, `live_caps` in `crates/hangar-engine/src/live.rs`. When they change, it
changes with them.

| Capability (`caps`) | Claude Code (with its mod) | Codex (attached) | Either, without a side channel |
|---|---|---|---|
| `turns`: working, waiting, done | reported, live | reported, live | guessed from the output, title, bell and notifications |
| `inject`: Hangar's messages | submitted when idle, added to a running turn | started when idle, steer a running turn | typed when idle; a running turn waits |
| `briefing.role`: orchestrator, worker, project manager | end of the system prompt | developer instructions | Claude Code: a labelled first message; Codex: developer instructions still |
| `briefing.state`: board, plan, latest events | session context, at each start and compaction | session context, at each start and compaction | with the first message |
| `approvals`, `questions` | held until you answer, from anywhere | held until you answer, from anywhere | keys typed into the dialog, checked on screen |
| `transcript` | `~/.claude/projects/…/<session>.jsonl` | `$CODEX_HOME/sessions/…/rollout-….jsonl` | the same files |
| `sessions` | resume, resume the last, fork; id reported | resume, resume the last, fork; id reported | id found in the history folder |
| `usage.context` | from the transcript | from the protocol, live | from the history files |
| `usage.limits` | reported by the mod | from the protocol, exact | Claude Code: unknown; Codex: from its session files |
| `mcp` | a file per launch (`--mcp-config`); a custom set keeps the others out | `-c` overrides, never written to Codex's configuration; a custom set turns the others off | the same |
| `extensions`: what a [kit](../kits.md#claude-code-and-codex) brings | MCP servers, skills, hooks, commands, agents, output styles, at each launch | MCP servers at each launch; skills and hooks once added to Codex; no commands, agents or output styles | the same |
| `accounts` | a folder per account (`CLAUDE_CONFIG_DIR`) | a folder per account (`CODEX_HOME`) | the same |
| `images` | takes images in | generates images, takes images in | the same |
| `headless`: reviewer, `delegate` | one-shot (`claude -p`) | one-shot (`codex exec`) and its app-server | the same |
| `launch` | model, extra folders, session name, allowed tools by any rule | model, reasoning effort, extra folders (as writable roots), Hangar's own tools allowed | the same |
| `permission_modes` | Accept edits, Auto, Bypass | Accept edits, Auto, Bypass (mapped as above) | the same |
| `background_jobs`: commands that outlive a turn | yes | yes | the same |

## Known limits

- **Written from the protocol, not from a session.** Where the schema says nothing of
  behaviour, the code relies on what Codex's source shows, and these are the first things a
  Codex bug report is read against: that Codex's own terminal (`codex --remote`) draws a turn
  Hangar started; that `codex --remote … resume <id>` resumes a session; that when two
  clients can answer one approval, the first answer wins and the other's dialog closes; and
  how a generated image is written in the newer format of Codex's session files.
- **Windows.** Hangar does not attach to Codex on Windows yet, in a native pane or a WSL one:
  Codex panes there run on the plain terminal.
- **Kits.** Codex cannot load a kit's skills and hooks for one session, so **Add to Codex**
  installs them for every Codex session on the machine, until you remove them; Codex runs a
  kit's hooks only once you trust them in its `/hooks`. A kit's commands, agents and output
  styles are Claude Code's only. [Kits](../kits.md#claude-code-and-codex).
- **The playbook.** The MCP panel's **Write the agent playbook** writes `CLAUDE.md`, and
  Codex reads `AGENTS.md`. Codex orchestrators and workers get their role from Hangar at
  launch either way.
- **No session name.** A Codex pane cannot be given a session name other sessions reach it
  by, as Claude Code's `--name` does.
