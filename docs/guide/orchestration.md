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
   dependencies and the files it owns. When a task is created, HangarAI reads its title and
   description and leaves it a comment with the files it should own, found in the code: the
   files and folders it names, the files that define the names it mentions, the file that
   registers a new one. The owns stay as the orchestrator wrote them; the comment is a
   suggestion.
2. **Start workers.** `worker_spawn` gives a task to a new agent pane: the workspace's
   worker agent (the orchestrator's, unless you chose another), or the one the orchestrator
   asks for, so a team can mix Claude Code and Codex. The worker runs in its own git
   worktree, on a new branch from the target branch, with a block of 20 ports for its dev
   servers and tests.
   There is no limit on how many workers run at once, or on how many times a task is
   tried: the orchestrator decides, and hands a task back to you when it needs you.
   HangarAI still refuses, whatever the prompt says, a task whose files overlap a task
   already in progress.
   - **Shared files.** A file that every task adds to, a registry or a dictionary, can be
     listed in `.hangar/shared` (one pattern a line, kept beside the board): tasks may
     add lines to it at the same time, and only a line removed there is pointed out to the
     reviewer. HangarAI suggests such files from the repository's history.
   - **Mechanical fixes.** A worker may make a one-line mechanical fix outside its files,
     such as a new field in a struct literal its change requires; it lists it, and the
     reviewer checks that it is only that.
   - **No waiting on a dependency.** A task whose dependencies are delivered but not landed
     yet starts at once: its worktree is made from the target branch with their branches
     merged in. When they land, HangarAI merges the target branch into the task's and tells
     its worker, which cannot deliver before them. The orchestrator hears at once when a
     task can start.
   - **Panes.** A workspace holds 40 panes by default, its terminals and views together,
     and up to 100 under **Panes per workspace** in its orchestration settings. Past 16, a
     new pane also needs 1.5 GB of free memory and processors that are not saturated.
   - **Model and effort.** The orchestrator picks a model and an effort for each worker
     (`worker_spawn {model?, effort?}`): Haiku for mechanical, well-specified edits (i18n
     keys, renames, a one-line fix with a clear test), Sonnet for routine features, tests,
     docs and small UI changes, Opus for design, servers, security, concurrency and changes
     across several parts, Fable for the hardest work and for a task a previous attempt got
     wrong. The effort is high by default, xhigh for hard or risky work, medium or low for
     trivial edits, and a retry goes up a level. A task can carry a suggestion
     (`board_create_task {model?, effort?}`), used when the orchestrator names none; with
     nothing chosen, a worker runs Sonnet at high (on Codex, Codex's own model). Codex has
     no xhigh or max: they run as high. A model or an effort the worker's agent does not
     take is refused with the list, rather than starting a pane that would stop on it. What
     a worker runs shows as « Sonnet · high » beside its header's Worker badge, on its
     task's card (the task's suggestion before a worker takes it), in `worker_list` and in
     the Agents tile.
     In the orchestration settings, **Workers** (the model) and **Workers' effort** are
     *Auto (the orchestrator picks)* by default. A model or a level chosen there is fixed
     for every worker of the workspace and wins over the orchestrator's choice, which its
     briefing says, so it does not try. A workspace that had a workers' model other than
     Sonnet before keeps it, fixed; one on Sonnet, the old default, is on Auto.
   - **Lean workers.** A Claude Code worker starts without four tools it never uses
     (Workflow, ReportFindings, ListAgents, SendFeedback), waits for its long builds and
     checks to end instead of polling them, and the board answers its writes with a short
     receipt rather than the whole task: fewer tokens on every request, nothing lost
     (`board_get_task` reads a task whole).
3. **Deliver.** A worker hands in its branch with `delivery_submit`. A delivery goes
   through four steps:
   - the branch is merged in a temporary worktree;
   - the workspace's check command (`pnpm test`, `cargo test`…) runs there, if you set
     one;
   - a separate reviewer agent reads the change with a fresh context and read-only tools
     (`claude -p`, or `codex exec` in a read-only sandbox);
   - if all of that passes, the branch lands on the target branch by fast-forward.

   If a step fails, the worker receives what failed and fixes it. HangarAI never pushes,
   and a landing that would overwrite your uncommitted changes is held back.

   **Finished work lands together.** Deliveries do not wait in line. When one is ready,
   HangarAI waits for the others arriving, until 15 seconds pass without a new one (a
   minute at most), then takes every delivery ready, up to 16 (**Deliveries checked
   together** in the orchestration settings): it merges them one after the other, runs the
   check once on the result, and has each one reviewed at the same time, every reviewer in a
   folder of its own, beside the check. A red check is narrowed down by halves to the
   deliveries that break it; only those go back, and the others land. A worker whose
   delivery failed its check and got requested changes receives both in one message. Ten
   ready deliveries with a 30-second check and 20-second reviews land in about 65
   seconds, where one after the other took 500.
   - **The merge train.** Deliveries that touch the same files land in the order they
     came. When two of them only add lines at the same place, both additions are kept; any
     other conflict goes back to its worker.
   - **A flaky check** is run once more when it fails: if it then passes, the delivery
     lands and its card says the check was retried. A test the change itself touched is
     never given a second run.
   - **Merged by hand.** A branch you merged into the target yourself, by a merge, a
     fast-forward or a squash, counts as landed: even while its delivery is queued or being
     checked, HangarAI sees it (it looks every 10 seconds), ends the check and the review,
     and records the delivery as landed by hand. A task already done stays done: a late
     result moves nothing, and a worker that submits for it is told it is over.
   - **A red check that is not the branch's fault.** When the check's machine is gone (no
     build machine, a lost connection, a full disk, no network), nothing is merged and nobody
     is blamed: the delivery runs again by itself after 1 minute, then 3, then every 10, the
     worker is told nothing, and the orchestrator hears it once when the machine stays down,
     and once when it is back. When the target fails the same check without the branch (the
     same failing tests, or the same errors), the branch is not blamed either: the worker is
     told once that it has nothing to fix, the orchestrator hears "waits on main: main fails
     its own check" once per commit of the target, and the delivery is tried again every
     minute, until the target moves. A branch that adds a failure of its own is still told
     what it broke.
   - **Waiting for your go.** With **Merge by itself** off, a delivery is checked and
     reviewed with the others, never holding them back, then waits for you in Needs you; it
     lands on your go without a second review, and its check runs again only if the branch
     moved meanwhile.

   Each workspace picks how much a delivery waits before it lands, its **delivery speed**:

   | Mode | Before it lands | After it lands | If the check is red |
   |---|---|---|---|
   | **Fast** | nothing: the worker's commits are the delivery | the check runs in the background | a fix task goes on the board; nothing is reverted |
   | **Balanced** (the default) | merged; reviewed only if risky | the check runs | the merge is reverted (`git revert`) and the worker fixes it |
   | **Safe** | merged, checked and reviewed | — | nothing landed; the worker fixes it |

   - **Balanced** lands a delivery as soon as it merges, so its work is on the branch
     within seconds and what depended on it can start; the orchestrator hears "landed —
     checking". A red check undoes it with a new commit (never a reset or a force push)
     and sends it back to its worker with the output; the next delivery puts the change
     back with the fix. Only a risky change is reviewed first: one that touches
     licensing, billing or payments, sign-in, secrets, database migrations, release
     scripts or CI workflows, deletes files, or changes more than 400 lines. The task's
     card says why it was reviewed, or why not ("small change, no risky path"). Every
     workspace is Balanced unless you chose another mode for it.
   - **Fast** starts workers in the workspace's folder (the orchestrator can still ask for
     a worktree) and never makes anyone wait: a delivery is done the moment it is
     submitted, and the check runs after it. When it fails, HangarAI adds a task to fix it,
     with the check's output, and tells the orchestrator. Your folder is never reverted.
   - **Safe** checks and reviews every delivery before it lands, as HangarAI did before
     3.0.

   The mode is chosen where the workspace's agents are: in the new-workspace dialog and in
   the workspace's orchestration panel, under **Delivery speed**. Balanced's risky paths
   (one pattern per line, empty for HangarAI's list) and its line threshold are under
   **Advanced** (in the settings file: `deliveryMode`, `riskPaths`, `riskLines`). The
   orchestrator is told the workspace's mode in its briefing. A delivery's chip and the chat
   view's cards say what the mode did: "Landed · checking…", "Reverted after a red check"
   (the check's output a click away), "Review skipped · low risk" and why, "Fix task
   opened".

   **Checks on HangarAI Cloud.** Under **Run delivery checks on**, a workspace can send its
   checks to [Cloud Builds](cloud.md) instead of this computer: each delivery is still
   merged here, then checked on a 32-thread machine rented for you alone, billed by the
   second in HangarAI credits and kept up 5 minutes for the next check; deliveries checked
   together share one run. The panel shows what a delivery's check costs, from the last
   ones. The delivery's card follows it (queued, looking for a machine, starting, running),
   and the task keeps its receipt. When HangarAI Cloud cannot take a check, it runs on this
   computer and the task's card says why, once; when the credits run out, **When the
   credits run out** chooses between checking here and waiting for credits, up to the
   check's time limit.
4. **Wake on events.** The orchestrator does not poll. When it is between turns, HangarAI
   sends it one batched message: a worker commented, is waiting, idle or exited; a delivery
   landed or failed. Only what is urgent comes within seconds: deliveries, a worker
   waiting for you or gone, a task that needs you, is blocked, can start or is done, your
   own comments and the ones that ask a question or say blocked. A worker is *waiting for
   you* only while a real dialog is up in its pane (a permission, a question): the message
   quotes what it asks, and when the dialog closes, a later message takes the wait back. One fact is one event: the same event of a batch is told once with its
   count, and nothing is said of a session that ended after its task moved on. The rest (plain comments,
   idle workers) comes as one digest at most every 5 minutes, or with the next urgent
   message, several comments on one task folded into one line. Nothing is dropped: the
   board and the orchestrator's journal keep every event.

   A worker is called idle when it ended its turn 10 minutes ago without delivering and
   has nothing of its own running in its pane: one waiting on a build, a benchmark or a
   CI watch it started in the background is not idle until that ends (on this machine;
   for a pane on a node HangarAI cannot look). It is called idle once, and again only after
   it did something (a comment, progress, a commit, a delivery) or 30 minutes later.
5. **Needs you.** One list, across workspaces, shows what waits for you:
   - agents asking for a permission or an answer, with what the dialog asks on the card;
   - questions the orchestrator handed back to you;
   - deliveries ready to merge, when automatic merging is off;
   - an agent that wants to spend HangarAI credits past its workspace's daily budget (100
     credits by default) or to publish a share link: *Allow once*, *Allow 200 more today*
     or *Deny*, and the agent is told your answer. For a machine it wants to rent
     ([`cloud_run`](cloud.md#machines-for-agents)), the card shows the machine, the command
     and the most the run can cost;
   - an agent stopped at its account's limit, with a button to restart it on another
     account, on the same conversation.

You set the following per workspace: the models of the orchestrator, workers and reviewer
(Opus, Auto and Sonnet by default for Claude Code; for Codex, a model id each, or Codex's
own default, Auto for the workers), the workers' effort (Auto by default), the check command, the target branch, automatic merging (on by default), the
permission mode, the delivery speed (Balanced unless you choose another), how many
deliveries are checked together, where checks run, and how many panes the workspace holds.
Which agent each role runs where a workspace chose none is in **Settings →
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

## Night shift

Leave a workspace a few objectives and a budget, go to bed, and find a report in the
morning. In the workspace's orchestration panel, **Night shift → Plan a night…** opens the
sheet:

- **Objectives**, one a line. The orchestrator gets them as its plan: it turns each into
  tasks on the board and starts workers on them, in parallel wherever their files allow.
- **Deliveries tonight**: Fast or Balanced. Every delivery lands by itself during the night
  (automatic merging is on for it); the workspace goes back to its own mode and merging
  setting in the morning, unless you changed them in the meantime.
- **Budget**: a stop time (07:00 by default, the next time the clock reads it), points of
  your plan's weekly window (15 by default: what a busy day takes), and the HangarAI credits
  the night may spend (none by default). The first limit reached ends the night, and every
  worker stops at once; shortly before (20 minutes before the stop time, or 85 % of a
  budget) the orchestrator is told to start nothing new, so that what runs lands.
- **While you sleep**, the night's promises. It never pushes, releases, publishes or
  deploys: a `git push`, a package published, a release, a deploy, a pull request opened
  or merged is interrupted the moment its pane reports it, and kept for you in the
  morning. Questions are parked, never waited on: a task blocked for you, an agent waiting
  for an answer or a permission is noted for the report and told to the orchestrator
  once, which carries on with the rest.

While it runs, the sheet shows where it stands (the stop time, what it spent, what landed,
what waits) and **End the night**; the orchestrator's pane wears a moon. The night also
ends by itself once every task of it is done or parked and nothing ran for 15 minutes.

The **morning report** waits as a card at the top right of the window, and as a
notification on your phones when Hangar Mobile is paired: what landed (the tasks, the
commits on the branch and the branches they merged), the pictures the deliveries produced
(the images the commits added, the screenshots and snapshots their comments name), what
waits for you, what did not make it and why, and what the night spent. Each line opens its
task, its pane or its picture. A night is kept in HangarAI's data folder
(`night/<workspace>.json`): a HangarAI started again during the night takes it up where it
was. Night shifts run on this computer's workspaces, with an orchestrator pinned.

## Delegating a sub-task

Some work is not a board task: "make the five icons", "have the other agent review this
function". Any agent hands it over with `delegate`, to Claude Code or Codex, which runs it
with no terminal, in the caller's folder or in a worktree of its own, and the result comes
back to the caller as a message when it ends. What the child asks you waits in **Needs you**
like any other question. [Codex (beta)](codex.md#delegate) has the details.

## Project manager

One agent to talk to instead of every project's agents (new in 1.0.3). Open it from the top
of the sidebar or the command palette: HangarAI creates an HQ workspace with its own folder and
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
