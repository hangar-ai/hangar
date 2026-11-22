# How agents use Hangar's MCP server

[← Hangar](../../README.md) · [Guide](README.md)

The MCP server is the app's own binary, started as `hangar-ai --mcp` over stdio. Each call
goes to the running app through a local HTTP API on `127.0.0.1`. The port and the token
change at each launch and are never written into agent configs. Every board change goes
through a single lock, so claims stay atomic across panes.

Claude Code and Codex call the same tools. For Claude Code, the MCP panel registers the
server in the project's `.mcp.json`, which Claude Code reads, and approves it once in
`~/.claude/settings.json` so it is not asked for again in every session. Hangar merges its
entry into the existing configuration rather than overwriting it, and keeps a backup the
first time it writes a file. To do it by hand instead, run
`claude mcp add hangar --scope project -- "<path to hangar-ai>" --mcp` in the project.

Codex has nothing to register: each Codex pane gets the server at every launch, as
`-c mcp_servers.hangar…` on the command line of the pane's Codex server, along with the
workspace's choice of servers and its kits' servers. Hangar never writes
`~/.codex/config.toml`.

| Tools | What agents do with them |
|---|---|
| `board_list_tasks`, `board_get_task`, `board_next_task`, `board_claim_task`, `board_create_task`, `board_update_task`, `board_comment_task`, `board_delete_task`, `board_report_progress` | Read the board, take the highest-priority free task whose dependencies are done, claim it (this fails if another agent holds it), split work into new tasks, move tasks between columns, and leave notes for each other. An agent says how far along its task in doing is with `board_report_progress`: a percentage and a short note, which you see on the board. To spare the agents' tokens, `board_list_tasks` gives a summary of each task, the most recently updated first and 50 at most (`limit`): its column, assignee, labels, dependencies, files, progress, how many comments and the start of the last one; tasks in done only when asked (`include_done`, or `column: "done"`). `board_get_task {id}` gives one task whole, its description and every comment. The writes answer with a short receipt (the task's id, title, column, assignee, labels, dependencies, progress and comment count) rather than the whole task. The board in HangarAI shows everything, as before. |
| `board_block_task`, `board_unblock_task` | Put a task on hold for something outside the board (a release, a decision, a date) with a reason and an optional `recheck` day, instead of a comment: it returns to todo with nobody on it, `board_next_task` and `worker_spawn` skip it, and its dependents keep waiting. The recheck day passed, it unblocks itself; `human_only` blocks are lifted by a person from the board, never by an agent. |
| `memory_search`, `memory_read`, `memory_write`, `memory_update`, `memory_list`, `memory_delete` | Look up what earlier sessions learned before starting, and record decisions, pitfalls and conventions. Writing an existing title replaces that entry. |
| `workspace_list`, `workspace_window`, `pane_list`, `pane_create`, `pane_send`, `pane_read`, `pane_restart`, `pane_close`, `pane_focus` | See the other panes and their state, open another Claude Code, Codex or shell pane (in a worktree, for instance), type into a pane or queue a prompt until it is idle (for an agent Hangar follows: Claude Code with its mod, Codex attached), read its output, and show a pane to you. |
| `workspace_create`, `workspace_set_check` | Any agent can ask for a workspace for a repository the person named (`workspace_create {repo, name?, parent?}`): Hangar clones it, into `parent` when you say where (any absolute folder, on any drive: « put it on my second SSD » gives `E:/AnyPS5`; missing folders are created, and an existing empty folder is used; your Projects folder setting is not changed), creates the workspace and starts an orchestrator that reads the repository, installs its dependencies, sets the check command, runs the tests once and reports. A repository that is not yours is cloned only after you click Clone in Hangar, on a card that shows the destination; for your own, the agent just tells you the folder. The orchestrator sets its own workspace's check command with `workspace_set_check {command}`, and you see it in the orchestration panel. |
| `worker_spawn`, `worker_list`, `worker_stop`, `delivery_submit`, `delivery_approve`, `delivery_status` | Orchestration. The orchestrator starts, lists and stops workers, of either agent (`agent`), each on the model and the effort it picks for the task (`model`: haiku, sonnet, opus, fable or a Codex model id; `effort`: low, medium, high, xhigh, max), unless the workspace fixes them; a task can suggest both (`board_create_task {model?, effort?}`). A worker hands in its branch for check, review and merge. A worker that sets `needsApproval` on `delivery_submit` (visual work, or a task that asks for it) has its delivery checked and reviewed with the others, never holding them up, but landed only once you give your go: press **Approve** on its card in **Needs you**, or tell the orchestrator, which relays it with `delivery_approve` (only when you said so). Already passed, it lands at once, checked again first if the branch moved. |
| `delegate`, `delegation_status`, `delegation_cancel`, `delegation_list` | Hand a sub-task that is not a board task to Claude Code or Codex, run with no terminal, in the caller's folder or a worktree of its own; follow it, stop it, list the workspace's. The result comes back as a message when it ends. [Details](codex.md#delegate). |
| `image_generate`, `image_edit` | Draw images from a prompt (a hero image, an illustration, an icon, the pictures of a mock-up), 1 to 4 a call, or change an image of this computer as the prompt says ("make the sky warmer"), with HangarAI's hosted image models. Each image is saved as a PNG (in `.hangar/images/` unless the agent names a place, never over an existing file or the original) and opens as a pane of the grid while it is drawn. Paid with your [HangarAI credits](#tools-that-cost-credits-or-ask-first). |
| `share_publish`, `share_list`, `share_delete` | Publish a file at a link anyone who has it can open for 30 days (`hangarai.dev/s/…`), for you to send to someone: a page (`.html`, with the images, video and audio beside it; its scripts and styles inline), a Markdown document (published rendered), a PDF, an image, or a 3D model in glTF; 100 MB at most. An agent publishes only when you ask for a link, and [asks you first](#tools-that-cost-credits-or-ask-first). `share_list` lists your license's links, live first, with who published each; `share_delete` ends, at once, a link the agent published itself (the ones you or another agent published are yours to delete, in the Cloud space; nothing is given back for the days left). |
| `cloud_classes`, `cloud_suggest`, `cloud_run` | [Cloud machines](#cloud-machines): the catalog of HangarAI Cloud's machines (CPU and GPU, their rates now and how many are free), the smallest one a piece of work needs, and a command run on one, from the agent's folder, its output and receipt back. Paid with your [HangarAI credits](#tools-that-cost-credits-or-ask-first). |
| `preview_show`, `preview_close` | Show you a file (an image, a video, a PDF, a page, Markdown, code or text) as a pane of the workspace's grid, from any folder of your computer, and close it. |
| `model_show`, `model_snapshot`, `model_close` | Open a 3D model from any folder of your computer as a pane of the workspace's grid, render it to an image to check it, and close it. |
| `kit_tab_list`, `kit_tab_open`, `kit_tab_close` | List the tabs the workspace's enabled [kits](../kits.md) bring, open one as a pane of the grid with its params (the page of a CI run, with the run's id), and close it when done. A kit with changes waiting for your review opens no tab until you accept them. |
| `phone_share` | Send you a file on your phone through [Hangar Mobile](mobile.md): a screenshot, a render, a report you asked to see. |
| `project_list`, `project_status`, `project_send` | The [project manager](orchestration.md#project-manager) only: list every project, read where one stands, and queue an instruction for its orchestrator. |

## Tools that cost credits or ask first

Four tools spend your license's HangarAI credits, and each answer says what it cost and
what is left. No other tool spends any of its own (a delivery checked on a Cloud Builds
machine is paid as the workspace's setting says: [Cloud Builds](cloud.md)).

| Tool | What it costs | Asks you first |
|---|---|---|
| `image_generate` | Per image of the default size (about a megapixel): **3** credits with `draft` (FLUX.1 [schnell]), **11.9** with `standard` (FLUX.2, the default), **39.4** with `best` (Nano Banana). | Past the workspace's daily budget for agents. |
| `image_edit` | **39.4** credits an image (Nano Banana). | Past the workspace's daily budget for agents. |
| `share_publish` | About **0.8** credit for the 30 days, per started 50 MB. | Always, unless the workspace lets its agents publish. |
| `cloud_run` | The machine's rate now, by the second, as for your own runs ([Prices](cloud.md#prices); `cloud_classes` gives the rates of the moment): a GPU machine costs several times a CPU one. The budget counts the most it can cost (its limit on the rate for the time limit, the start and the keep) until its receipt. | Past the workspace's daily budget for agents. |

The rates are the server's: the tools' descriptions give the agent those of the moment, so
it can choose. Each workspace gives its agents a daily budget of credits (100 unless you set
another). Within it, an image goes at once; past it, and before any share link, the agent
asks you in **Needs you** ("Ava wants to generate 4 images · 47.6 credits"): *Allow once*,
*Allow 200 more today* or *Deny*. Nobody has to be at the window: after 40 seconds the tool
answers that it waits for you, with nothing spent, the agent does something else, and
Hangar tells it your answer; it calls again to go ahead. On too few credits the tool says so,
and you can buy more in Settings › License. `share_list` and `share_delete` cost nothing.

## Cloud machines

`cloud_classes`, `cloud_suggest` and `cloud_run` let an agent run work on a HangarAI
Cloud machine ([Cloud Builds](cloud.md#machines-for-agents)) when this computer is too
slow for it, or has no GPU it needs. Their descriptions teach the agent to pick the
smallest machine that fits, and to ask for a GPU only when the work runs on one.

- `cloud_classes {any_region?}`: each class with its id, what it is for, its hardware,
  its rate now and its limit, how many are free; the GPU models a spec may name; whether
  the license may rent a GPU (credits are the only limit: no hours).
- `cloud_suggest {task, gpu?, gpus?, vram_gb?, cpus?, ram_gb?, disk_gb?, minutes?}`: the
  agent says what the work runs and what it knows of its needs; the answer is the
  smallest class that has it (one with a machine free now first), or an advanced spec
  when no class has it, with its rate, what `minutes` of work likely cost, the most it
  can cost with the time limit it suggests, why, and the arguments to give `cloud_run`.
  A GPU only when the work runs on one (CUDA, training or running a model, GPU
  rendering); a model's size named in the task ("13B", "in 4-bit") gives the GPU memory
  it needs. It spends nothing.
- `cloud_run {command, class | gpu, gpus, vram_gb, cpus, ram_gb, disk_gb, time_limit?,
  folder?, any_region?, wait?, approval?}`: the command runs from the agent's folder (or
  `folder`), as `hangar-ai cloud run` runs it: the files git lists are sent, never
  `.git`, ignored files nor a file named like a secret. It goes through the budget with
  the most it can cost; past it you are asked in **Needs you**, with the machine, the
  command and that amount, and the tool answers `waiting_for_approval` until you decide.
  The call waits up to 45 seconds for the end and answers with what the command printed
  (its last 8000 characters), its exit code and its receipt; a run still going answers
  `running` with its id (`cr-…`): `cloud_run {run}` waits more and gives what it printed
  since, `{run, stop: true}` stops it, and Hangar tells the agent in its pane when it
  ends. The whole output stays on this computer: `hangar-ai cloud logs <job>`.

The acceptable use policy of HangarAI Cloud is accepted by you, once, in the app; an agent
is told to ask you. A license without GPU machines gets a sentence saying so, and nothing
is rented.

What leaves the computer: for an image, the prompt (and, for an edit, the image) goes to
HangarAI's server and to fal.ai (United States), which keep neither. A share link's files go
to HangarAI's server and to Cloudflare R2 in the EU, and are deleted when the link ends;
its visitors are not tracked.
