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
| `board_list_tasks`, `board_next_task`, `board_claim_task`, `board_create_task`, `board_update_task`, `board_comment_task`, `board_delete_task` | Read the board, take the highest-priority free task whose dependencies are done, claim it (this fails if another agent holds it), split work into new tasks, move tasks between columns, and leave notes for each other. |
| `board_block_task`, `board_unblock_task` | Put a task on hold for something outside the board (a release, a decision, a date) with a reason and an optional `recheck` day, instead of a comment: it returns to todo with nobody on it, `board_next_task` and `worker_spawn` skip it, and its dependents keep waiting. The recheck day passed, it unblocks itself; `human_only` blocks are lifted by a person from the board, never by an agent. |
| `memory_search`, `memory_read`, `memory_write`, `memory_update`, `memory_list`, `memory_delete` | Look up what earlier sessions learned before starting, and record decisions, pitfalls and conventions. Writing an existing title replaces that entry. |
| `workspace_list`, `workspace_window`, `pane_list`, `pane_create`, `pane_send`, `pane_read`, `pane_restart`, `pane_close`, `pane_focus` | See the other panes and their state, open another Claude Code, Codex or shell pane (in a worktree, for instance), type into a pane or queue a prompt until it is idle (for an agent Hangar follows: Claude Code with its mod, Codex attached), read its output, and show a pane to you. |
| `workspace_create`, `workspace_set_check` | Any agent can ask for a workspace for a repository the person named (`workspace_create {repo, name?, parent?}`): Hangar clones it, into `parent` when you say where (any absolute folder, on any drive: « put it on my second SSD » gives `E:/AnyPS5`; missing folders are created, and an existing empty folder is used; your Projects folder setting is not changed), creates the workspace and starts an orchestrator that reads the repository, installs its dependencies, sets the check command, runs the tests once and reports. A repository that is not yours is cloned only after you click Clone in Hangar, on a card that shows the destination; for your own, the agent just tells you the folder. The orchestrator sets its own workspace's check command with `workspace_set_check {command}`, and you see it in the orchestration panel. |
| `worker_spawn`, `worker_list`, `worker_stop`, `delivery_submit`, `delivery_status` | Orchestration. The orchestrator starts, lists and stops workers, of either agent (`agent`). A worker hands in its branch for check, review and merge. |
| `delegate`, `delegation_status`, `delegation_cancel`, `delegation_list` | Hand a sub-task that is not a board task to Claude Code or Codex, run with no terminal, in the caller's folder or a worktree of its own; follow it, stop it, list the workspace's. The result comes back as a message when it ends. [Details](codex.md#delegate). |
| `preview_show`, `preview_close` | Show you a file (an image, a video, a PDF, a page, Markdown, code or text) as a pane of the workspace's grid, from any folder of your computer, and close it. |
| `model_show`, `model_snapshot`, `model_close` | Open a 3D model from any folder of your computer as a pane of the workspace's grid, render it to an image to check it, and close it. |
| `phone_share` | Send you a file on your phone through [Hangar Mobile](mobile.md): a screenshot, a render, a report you asked to see. |
| `project_list`, `project_status`, `project_send` | The [project manager](orchestration.md#project-manager) only: list every project, read where one stands, and queue an instruction for its orchestrator. |
