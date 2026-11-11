# Discord connector (experimental, design)

A Discord server as another surface of Hangar, for whoever would rather not
open the app: a category per folder, a channel per workspace (the
conversation with its orchestrator, as the chat view has it). The workers
stay out of sight: a line when one starts or delivers, and their card when
they need the person. A bot of the person's own reports what happens and
carries what the person says and decides back to the panes; only the person
is heard. Workspaces on nodes work the same. Nothing here is built yet: this is
the design, the research it rests on, and mockups of every card.

Experiment: `discord-connector`.

![The orchestrator's channel](../images/discord/1-conversation.png)

## Where it runs

The state of the workspaces, the orchestration, the boards and the
deliveries live in `hangar-app`; `hangar-node` only runs terminals for a
desktop (nodes.md). So the bot runs inside the desktop app, as Hangar Mobile
does, and is one more client of `mobile::desk`: the calls the chat view and
the phone already make (read the conversation, send, answer a question,
allow a permission, approve a plan, queue and take back, stop, land or send
back a delivery). Every answer goes through the hangar-bridge mod as the
chat view's do.

### Nodes

A workspace on a node is a channel like any other: the desktop already
reads its conversation, its board and its deliveries through the node, and
`mobile::desk` reaches its panes the same way. What the connector needs from
nodes:

- taking a prompt back from the queue of a pane on a node (`Port::unqueue`
  refuses it today: a route on the node API, `bridge_cancel` there);
- with that, the queue and the model / effort / plan settings for such panes
  (the chat view leaves them out for now);
- answers to a node's panes are typed as keys (the held asks live in the
  node's core); answering them through the node's mod is a later step.

The desktop drives its nodes over SSH, and the orchestration (events to the
orchestrator, workers started, state of the workspaces) runs in the desktop.
So the bot answers while Hangar runs on the PC, window closed or not. A
Hangar entirely on a server, the PC off, needs the orchestration on the node
first (nodes.md, "Later"); the connector is written against `mobile::desk`
so that it can move there with it.

## Mapping

| Hangar | Discord |
|---|---|
| Folder | Category (up to 50 channels each) |
| Workspace, local or on a node | Text channel: the orchestrator's conversation |
| Worker | Nothing of its own: a line when it starts or delivers, and its permission, question or plan card in the workspace's channel when it needs the person, under its name |
| Agent (Leo, Ada…) | The bot's own webhook in the channel, with the agent's name and a generated avatar per message |
| Hangar's news | Short lines with Hangar's icons, as in the chat view |
| State of the workspace | One pinned dashboard card per channel, edited as it changes |

Decided with the person: no thread per worker; only the paired account is
heard; workspaces on nodes are in.

## Messages: Components V2

Every message the bot sends uses Components V2 (message flag
`IS_COMPONENTS_V2`, `1 << 15`; it cannot be removed once set, and it
excludes `content`, `embeds`, `poll` and stickers).

| Component | Type | Used for |
|---|---|---|
| Container | 17 | Each card, its accent bar in Hangar's state colours: working `#5fd27f`, waiting `#e7c06b`, accent `#3e8bff`, failed, done grey |
| Text Display | 10 | Markdown: headings, lists, code with highlighting, `-#` subtext, mentions, `<t:…:R>` times |
| Section | 9 | 1–3 texts with a Button or a Thumbnail beside them: question options, dashboard rows |
| Separator | 14 | Spacing, with or without a line |
| File | 13 | `plan.md`, a delivery's `.diff` (`attachment://` only; 20 MiB) |
| Media Gallery | 12 | Screenshots (1–10 items) |
| Action Row + Button / selects | 1, 2, 3… | Up to 5 buttons, or one select of up to 25 options |

At most 40 components per message. Libraries cite about 4000 characters of
text per V2 message; the official docs do not state it: to confirm, and
split long answers below it at paragraph or code block boundaries.

Modals: a Label (18, label ≤ 45, description ≤ 100) around a Text Input,
a select, a File Upload (19), a Radio Group (21), a Checkbox Group (22) or a
Checkbox (23); 1–5 components, title ≤ 45. A modal is the answer to an
interaction only.

## The cards

**A turn.** One card per turn of the orchestrator, accent working: its calls
as compact lines with Hangar's icons (application emojis: an app owns up to
2000, usable anywhere), the model and context, and Stop. Edited at most every
~2 s while it runs; when the turn ends it folds to one line ("12 actions ·
6 minutes") and Claude's answer is a message of its own. `<t:start:R>`
("1 minute ago") updates on the client without an edit. While it works, the
typing indicator (`POST /channels/{id}/typing`, 10 s) is renewed.

**A permission.** Accent waiting, the command in a code block, Allow
(success) and Deny (danger), the person mentioned. Answered, the buttons
give way to "✅ Allowed by @you <t:…:R>" and the accent turns grey.

![Permission and question](../images/discord/2-permission-question.png)

**A question.** One question: a Section per option with Choose beside it, or
a string select for several answers; "Other answer…" opens a modal. Several
questions (AskUserQuestion asks at most 4): "Answer the N questions…" opens
one modal, radio groups and checkbox groups plus a free text field.
twilight 0.18 has no Radio Group yet: a string select in a Label does the
same.

![Several questions](../images/discord/3-questions-modal.png)

**A plan.** The plan as Markdown in a waiting card; past the text limit, an
excerpt and the whole plan as `plan.md`. Approve the plan, and Keep
planning… which opens a modal for what to change (the chat view's plan card,
`held::plan_decision`).

![A plan to approve](../images/discord/4-plan.png)

**A delivery.** Its files in a ```` ```diff ```` block (Discord colours `+`
lines green and `-` lines red), the check and the review, the branches, the
`.diff` as a File, and Land / Send back… (a modal for what to fix). Landed:
a green one-line card.

**The dashboard.** Pinned, edited at most every ~5 s: the orchestrator and
each worker with its state and since when, a link to the card of whoever
waits on the person, and the counts of the board.

![Delivery and dashboard](../images/discord/5-delivery-dashboard.png)

**A worker that needs the person.** Its card in the workspace's channel,
under its own name, saying which task it works on and, on a node, which
one; answered there like the orchestrator's.

![A worker on a node that needs the person](../images/discord/6-node-worker.png)

**Commands.** Registered on the guild only (so they show at once):
`/stop`, `/model`, `/effort`, `/plan`, `/status` (ephemeral), for the
channel's orchestrator.

## What the person writes

Read with the `MESSAGE_CONTENT` intent (privileged; a toggle in the
Developer Portal under 10,000 users). Sent while the agent works, it waits
in the bridge queue: the bot reacts ⏳, then ✅ when it is taken. Deleting
the Discord message takes it back from the queue; editing it changes what
waits. Only the paired account is heard: anyone else's messages are
ignored, and their clicks get an ephemeral "only … can answer here".

## Interactions

Answer within 3 s or the interaction fails: a button that reaches the pane
is acknowledged at once with `DEFERRED_UPDATE_MESSAGE` (6), then the message
is edited (the token lasts 15 minutes). `UPDATE_MESSAGE` (7) where the new
card is known at once; `MODAL` (9) for the forms.

## Notifications

Work in progress is sent with `SUPPRESS_NOTIFICATIONS` (`1 << 12`). Only what
waits on the person mentions them (permission, question, plan, delivery
ready), with `allowed_mentions` limited to them: a push from Discord then
means something.

## Setup and security

- The person creates the Discord application and its bot, and the server: bots
  cannot create guilds since July 2025. Hangar gives the invite link with the
  scopes `bot applications.commands` and the permissions it needs (Manage
  Channels, Manage Webhooks, Send Messages, Embed Links, Attach Files, Read
  Message History, Add Reactions, Use External Emojis, Pin Messages, a
  separate permission since February 2026).
- The bot token stays on the PC with Hangar's other secrets.
- Pairing: Hangar shows a code, the person sends it to the bot; only that
  Discord account is heard afterwards. A private server is recommended: a
  message there can run commands on the PC.
- Channel obfuscation (mandatory from November 16, 2026): a bot no longer
  sees channels it cannot view, so Hangar gives itself explicit access on the
  categories it creates.

## Limits to respect

- Rate limits: read the response headers rather than hard-coding them;
  50 requests/s globally; 10,000 invalid requests (401, 403, 429) in
  10 minutes ban the IP for a while.
- No streaming: a message per answer of Claude's, the turn card edited in
  batches.
- Renaming a channel is heavily limited (about twice per
  10 minutes, community knowledge, not in the docs): no state in names; the
  state lives in the cards and the dashboard.
- Webhook names cannot contain `clyde` or `discord`; a webhook the
  application created can send interactive components, others cannot.

## Implementation

[twilight](https://github.com/twilight-rs/twilight) 0.18 (November 2026):
gateway, HTTP with its rate limiter, and a model with Components V2, Label,
File Upload, Checkbox Group and Checkbox (no Radio Group yet). serenity 0.12.5,
the latest stable release, has no Components V2. TLS through rustls, as the
rest of Hangar.

Steps, each behind the experiment:

1. Settings: the bot token, the guild, pairing the person's account.
2. A channel per workspace: the orchestrator's conversation both ways, and
   the permission, question and plan cards, the workers' included.
3. Deliveries with Land / Send back; the queue (⏳, delete, edit).
4. Following the structure (folders, workspaces created, renamed, closed),
   the dashboard and the slash commands.
5. Nodes: taking back what waits in a node pane's queue, and the settings
   of its panes.

Tested against a fake gateway and HTTP server; a real run needs the person's
bot.

## Sources

The official documentation, read from
[discord/discord-api-docs](https://github.com/discord/discord-api-docs)
(commit `36adecb`, November 10, 2026): `components/reference`,
`change-log`, `resources/message`, `resources/webhook`, `resources/channel`,
`topics/rate-limits`, `interactions/receiving-and-responding`.

## Mockups

`docs/images/discord/mockups.html` draws the six screens above in a page
styled after Discord's dark theme; `node docs/images/discord/shoot.cjs` turns
them into the PNGs. They are mockups, not screenshots of Discord.
