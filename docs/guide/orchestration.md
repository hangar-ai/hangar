# Orchestration and the project manager

[← Hangar](../../README.md) · [Guide](README.md)

## Orchestrator mode

A new workspace starts with one agent pane that is its orchestrator, Claude Code or Codex
(beta): the create dialog's *Starts with* offers the agents installed, Claude Code by default
when both are, and **More options** offers other setups. In an existing workspace, pin a
Claude Code or Codex pane as its orchestrator. It plans the work on the board and starts
workers to carry it out. You follow everything from one window.

**How it works**

1. **Plan.** The orchestrator breaks the goal into board tasks. Each task lists its
   dependencies and the files it owns.
2. **Start workers.** `worker_spawn` gives a task to a new agent pane: the workspace's
   worker agent (the orchestrator's, unless you chose another), or the one the orchestrator
   asks for, so a team can mix Claude Code and Codex. The worker runs in its own git
   worktree, on a new branch from the target branch, with a block of 20 ports for its dev
   servers and tests.
   There is no limit on how many workers run at once, or on how many times a task is
   tried: the orchestrator decides, and hands a task back to you when it needs you.
   Hangar still refuses, whatever the prompt says, a task whose files overlap a task
   already in progress.
3. **Deliver.** A worker hands in its branch with `delivery_submit`, and the branch joins
   a merge queue. Each delivery goes through four steps:
   - the branch is merged in a temporary worktree;
   - the workspace's check command (`pnpm test`, `cargo test`…) runs there, if you set
     one;
   - a separate reviewer agent reads the change with a fresh context and read-only tools
     (`claude -p`, or `codex exec` in a read-only sandbox);
   - if all of that passes, the branch lands on the target branch by fast-forward.

   If a step fails, the worker receives what failed and fixes it. Hangar never pushes, and
   a landing that would overwrite your uncommitted changes is held back.
4. **Wake on events.** The orchestrator does not poll. When it is between turns, Hangar
   sends it one batched message: a worker commented, is waiting, idle or exited; a delivery
   landed or failed. Only what is urgent comes within seconds: deliveries, a worker
   waiting for you or gone, a task that needs you, is blocked or is done, your own
   comments and the ones that ask a question or say blocked. The rest (plain comments,
   idle workers, unblocked tasks) comes as one digest at most every 5 minutes, or with
   the next urgent message, several comments on one task folded into one line. Nothing is
   dropped: the board and the orchestrator's journal keep every event.

   A worker is called idle when it ended its turn 10 minutes ago without delivering and
   has nothing of its own running in its pane: one waiting on a build, a benchmark or a
   CI watch it started in the background is not idle until that ends (on this machine;
   for a pane on a node Hangar cannot look). It is called idle once, and again only after
   it did something (a comment, progress, a commit, a delivery) or 30 minutes later.
5. **Needs you.** One list, across workspaces, shows what waits for you:
   - agents asking for a permission or an answer;
   - questions the orchestrator handed back to you;
   - deliveries ready to merge, when automatic merging is off.

You set the following per workspace: the models of the orchestrator, workers and reviewer
(Opus, Sonnet and Sonnet by default for Claude Code; for Codex, a model id each, or Codex's
own default), the check command, the target branch, automatic merging (on by default) and the
permission mode. Which agent each role runs where a workspace chose none is in **Settings →
General → Default agents**. Orchestration needs a git repository and an agent, Claude Code or
Codex.

<table>
  <tr>
    <td width="50%"><img src="../../.github/assets/needs-you.webp" alt="The Needs you list: one question the orchestrator handed back to you, which provider takes EU cards, and three deliveries ready to merge, whose check and review passed"></td>
    <td width="50%"><img src="../../.github/assets/orchestration-settings.webp" alt="A workspace's orchestration settings: permissions, automatic merging, here turned off, and the check command"></td>
  </tr>
  <tr>
    <td align="center"><sub>Needs you: what waits for you, across workspaces</sub></td>
    <td align="center"><sub>Orchestration settings of a workspace</sub></td>
  </tr>
</table>

## Delegating a sub-task

Some work is not a board task: "make the five icons", "have the other agent review this
function". Any agent hands it over with `delegate`, to Claude Code or Codex, which runs it
with no terminal, in the caller's folder or in a worktree of its own, and the result comes
back to the caller as a message when it ends. What the child asks you waits in **Needs you**
like any other question. [Codex (beta)](codex.md#delegate) has the details.

## Project manager

One agent to talk to instead of every project's agents (new in 1.0.3). Open it from the top
of the sidebar or the command palette: Hangar creates an HQ workspace with its own folder and
one agent, the one new orchestrators run (Claude Code on Opus by default), that follows every
workspace, with or without an orchestrator.

- **Ask it where your projects stand:** agents, boards, what waits for you, deliveries ready
  to land, each plan's status.
- **Ask it to pass an instruction on.** It is queued for that project's orchestrator, and a
  sleeping workspace wakes up.
- **It speaks up only for what waits for you:** an agent waiting for an answer or a
  permission, a task marked needs-you, a delivery ready to land, a board whose tasks are all
  done.

It keeps a tracker of what you want from each project, handed back after every compaction.
It never writes code and never acts on its own. On your phone, Hangar Mobile opens it as a
chat.
