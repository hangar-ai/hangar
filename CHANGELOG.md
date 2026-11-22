# Changelog

What changed in each HangarAI release (Hangar.AI until 2.4.5), newest first. The format
loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[Semantic Versioning](https://semver.org/); before 1.0.0, a minor version could
change behaviour as well as add to it. The 0.x history is grouped by minor version.

Every release, with its installers, is on the
[releases page](https://github.com/hangar-ai/hangar/releases).

## [Unreleased]

## [3.0.0] - 2026-11-22

HangarAI 3.0 is a paid app, with hosted features of its own, HangarAI Cloud, and an
orchestrator that lands finished work together in minutes. Installs of 2.5.1 and earlier keep
HangarAI for free.

### Added

#### Plans, credits and your account

- **Licenses.** A 14-day free trial: sign in with a HangarAI account (*Continue with GitHub*), no card asked, one trial per account. Then **HangarAI Pro**, 20 € a month or 192 € a year, with 3,000 credits a month, or **HangarAI Max**, 39 € a month or 374 € a year, with 10,000 credits a month and its Cloud Builds first in the queue, on up to 5 machines at once (Pro: 3). Move between Pro and Max at any time from **Manage subscription**, prorated. Prices include VAT; payments are handled by Polar. There is no Lifetime license. A license runs on 3 computers. Without a license HangarAI shows its license screen only, and your workspaces, board and memory wait for you untouched.
  - Already using HangarAI (2.5.1 or earlier)? Sign in to your HangarAI account at the first launch of 3.0 (*Continue with GitHub*, no card, nothing to pay) and your install gets a free license, held by your account, one per account: everything you had keeps working, with 30 credits a month for dictation. Then **Welcome to HangarAI 3.0** shows once what is new (one view in tiles and its layouts, the windows redone, the night shift, deliveries that land together, previews you act from and pasted images) and that your free license stays yours, with what Pro and Max add; **What's new in 3.0** in the command palette shows the new things again.
  - **Settings › License**: your plan, its renewal, its computers, your key, the rate of each hosted feature in credits and euros, **Upgrade to Max** and **Manage subscription**. A referral code can be given when you buy.
  - **Manage computers** frees a seat from your account. A license copied to more computers than it has seats is seen at its daily check; the copies then say so on a screen of their own, with *Manage computers* and *Try again*, never by stopping without a word.
- **HangarAI credits**, one balance for every hosted feature: Pro includes 3,000 a month, Max 10,000, the trial 150. Packs of 1,000 (3 €) and 5,000 (10 €) never expire. **Pay as you go**, off by default, keeps hosted features running past your credits up to a monthly limit you set. Every cost shows in credits with its euros beside it, at one rate across the app, before you spend.
- **Agents' spending.** What agents may spend on hosted features each day, per workspace (100 credits by default). Past it, and before any share link, an agent asks you in **Needs you**: *Allow once*, *Allow 200 more today* or *Deny*; it is told your answer and carries on.
- **Invite & earn.** Share your link from hangarai.dev/account and earn 30 % of what the people you bring pay, for as long as they pay; they get 1,000 credits.
- **Your HangarAI account** at hangarai.dev/account: your license and its computers, your credits and their history, pay as you go, your subscription and invoices at Polar, the downloads, and Invite & earn.

#### HangarAI Cloud

Hosted features that cost nothing until you use them, paid with your credits.

- **The Cloud space.** The cloud chip in the status bar, or **Show HangarAI Cloud** in the command palette: your balance; the machines and share links at work; the history of what each service spent, by day, filtered by service and period, with a receipt for every line. **Settings › Cloud** lists each service with its rate and what leaves this computer.
- **Dictation, hosted.** Hold the key and speak: the audio is transcribed on HangarAI's server (Groq's Whisper) and never stored. About 0.66 credit a minute, two minutes at most a dictation; a transcription that failed on the way can be tried again.
- **Image generation.** Ask an agent for an image: its `image_generate` and `image_edit` tools draw or edit one (*Draft*, *Standard* or *Best*). It opens in a preview pane while it is drawn, is saved in the workspace (`.hangar/images/`), and the agent's answer gives its path and what it cost. An image the model refuses or the safety filter holds back costs nothing. The conversation shows it as a card, the Cloud space as a line of its history.
- **Share links.** **Share link**, in a preview pane's header, publishes an image, a page, a PDF, a Markdown document or a 3D model, up to 100 MB, for 30 days: the sheet says what goes, until when and what it costs, then copies the link and shows its QR code. Anyone with the link opens it at hangarai.dev/s/…, served from share.hangarai.dev, apart from everything else, with a *Report* button. Extend a link up to a year, copy it or delete it from the pane's *Shared* chip or from the Cloud space. Agents publish with `share_publish` (after your OK, unless the workspace lets them), and list and delete their own links.
- **Cloud Builds.** Run a build, a test suite, a training run or a check on a machine rented for you alone, in Europe (Secure Cloud), billed by the second and destroyed afterwards. **Cloud: Run in the cloud…** in the command palette, or `hangar-ai cloud run -- <command>` in a terminal (with `cloud logs` and `cloud stop`): before it starts, a sheet shows the machine's rate, the files that go and the most the run can cost; the run gets a pane with its live output, and a receipt when it ends. Your dependencies are cached between runs, encrypted, for 14 days. Max's runs go first in the queue. For Pro and Max, as long as your credits pay, with no limit of hours; mining, proxies, scans and mail are not allowed.
  - **Delivery checks on HangarAI Cloud.** In a workspace's orchestration settings, *Run delivery checks on* **HangarAI Cloud**: each delivery is merged here and checked on a 32-thread machine, kept up 5 minutes for the next one, and deliveries checked together share one run. Its card says where the check stands (queued, looking for a machine, starting, running), and the task keeps the receipt. When credits run out, the check runs on this computer (said once) or waits for credits, as you choose.
  - **Machines for any need.** The sheet and `--class` offer classes: 32 or 64 CPU threads for builds and tests, 1 RTX 4090 (24 GB), 1 A100 80 GB or H100, or 2 to 8 GPUs for training and rendering, each with its rate now and how many are free (`hangar-ai cloud classes` lists them). **Advanced** says what the machine must have instead (the GPUs and their models, the memory of each, the threads, the memory, the disk): the sheet shows the machine it found and its rate or, when nothing free matches, the closest offers with what each one misses. A run's time limit goes from 15 minutes to 24 hours in the sheet (1 minute to 24 hours from a terminal), 30 minutes by default and 2 hours on a GPU. GPU machines are for Pro and Max, like every machine, with a watch against mining (a known miner, a pool's connection, a GPU held full for 10 minutes) that stops the run and holds Cloud Builds until a review. The machine you run on is the project's from then on.
  - **Choose what goes and where.** Europe first: a run leaves it only when you choose **Any region for this run**, said before it starts. A file named like a secret goes only if `.hangar/cloud-include` lists it, and an environment variable only if `.hangar/cloud.env` or `--env` names it. **Forget it**, or `hangar-ai cloud forget-cache`, deletes a project's dependency cache and its key at once. The machine stays up afterwards for none, 2 (the default), 5, 10 or 30 minutes, as the project's setting says.
  - **Agents pick the machine.** Claude Code and Codex have three tools: `cloud_classes` lists the catalog, `cloud_suggest` answers the smallest machine a piece of work needs (a GPU only when the work runs on one: a CUDA training job gets the RTX 4090, a `cargo test` 32 threads) with what it likely costs and the most it can, and `cloud_run` runs a command on it from the agent's folder and gives its output and receipt back. Within the workspace's daily budget for agents it starts at once; past it, **Needs you** asks with the machine, the command and the most the run can cost. The agent never accepts the acceptable use policy: you do.
  - **Big projects: data, caches and results.** A run can bring **data sets**, folders of any size kept out of git on your computer (games, datasets, models): `hangar-ai cloud data add <name> <path>…` makes one, `cloud data sync` sends it, `cloud data ls` and `cloud data rm` list and delete them, and `--data <name>` gives a run its files in their place in the project. They are cut into chunks and sealed on this computer with a key that never leaves it for HangarAI's server, sent once and then only what changed, and kept until you delete them. Named **caches** (`--cache ccache=.ccache`, `--cache pipelines/{gpu_model}/{driver}=.cache/shaders` for a shader cache per GPU model and driver) are restored before the command and saved after it, kept 30 days after their last use; what the command leaves in `$HGC_RESULTS` comes back as the run's **results** into a folder of yours (`--results <folder>`, or `cloud results <job>` later), kept 30 days. A machine kept between runs keeps what it fetched: a 200 GB game is downloaded once a machine. Storing costs 14.8 credits a GB-month, quoted before anything is sent; reading costs nothing. The server holds ids, sizes and ciphertext, never a name, a path or a byte. Agents name data sets in `cloud_run`; only you add, sync or delete one.
  - **Groups, priorities and your own image.** The runs of a **group** (`--group`) go to one machine, one after the other or side by side, and no other run shares it, so a benchmark is measured alone; it waits for the group's next run, up to a day. A run's **priority** (`--priority high|normal|low`) orders your own runs in the queue. A run can name **its own image** (`--image`), a private one with its registry sign-in, kept encrypted on the server and erased when the run ends. When a run brings data, the machine is chosen on what the run costs there, the host's traffic included.
  - **Full VMs.** The **VM** classes rent a whole virtual machine with its own Linux 6.8 kernel, root when the class says so, userfaultfd with write protection and `PAGEMAP_SCAN`, perf and bpftrace: for the work a container cannot run, such as a console emulator's game sessions. A VM is ready in about 2 minutes.
  - **Seen live.** The Cloud tile and the Cloud space show each run's group, priority, image and VM, its GPU and CPU while it runs, and the gigabytes coming down or going up as they move, whoever started it: you, an agent or a delivery check. The receipt keeps what moved and what the machine could do.
- **Hosted nodes.** **Keep running in the cloud…**, in a workspace's menu, rents a server for you alone at Hetzner, in Falkenstein, Nuremberg or Helsinki, puts a `hangar-node` on it and moves your workspaces there: their agents, board and memory keep working, and workers keep delivering, with your computer off. The sheet shows the size (Small, 2 vCPU and 4 GB, 34.0 credits a started hour; Standard, 4 vCPU and 8 GB, 61.4; Large, 8 vCPU and 16 GB, 119.4), until when (30 minutes to 24 hours from now), the place, the agents to install and the workspaces to move, and reserves the whole session at once: 11 hours on a Standard are 675.4 credits, and what you do not use comes back. A license runs as many at once as its plan runs machines, Cloud Builds' counted with them (Pro 3, Max 5); a session is extended for as long as your credits last, up to 24 hours ahead each time, with a reminder 30 minutes before the end, on the phone too. At the end the work comes back: straight over SSH when your computer reaches the node, otherwise parked, sealed to a key only this computer holds, in Cloudflare R2 for 7 days, and brought back by itself when HangarAI opens. For Pro and Max, not the trial; HangarAI's server never sees your code, your conversations or your agents' sign-in.
- **Hangar Mobile from anywhere**, without Tailscale: an end-to-end encrypted relay through HangarAI's server, which cannot read what passes, paired by QR code, for paid licenses and the trial. Settings › Mobile. The phone tries Tailscale first, then the relay.
- **Your computers.** HangarAI on your laptop drives HangarAI on your PC, on the same license: **Your computers**, in the navigator, lists your other computers with HangarAI open, and the PC's workspaces open on the laptop as if they were local, their terminals, conversations, board and deliveries included, while the work runs on the PC. Both connect out through HangarAI's relay, encrypted end to end; nothing listens on either. **Connect** shows six digits once, which you type on the PC, or on your paired phone in Hangar Mobile's **For you**, away from the PC. On the PC, **Driven from MacBook Air** with **Disconnect** for as long as it is driven; its own window stays usable, and it is kept awake while driven. A dropped link is dialed again by itself, and each terminal catches up with what it missed; a PC asleep or closed says so (« seen 3 h ago »). One terminal has one size, the size of the screen that last typed in it; the other draws it at that size, with **Take over**. **Settings › Computers**: this computer's name, *Show to my other computers* (on by default), the computers allowed to drive it with **Remove**, those it drives with **Forget**. For paid licenses and the trial.

#### Orchestration

- **Finished work lands together.** Every delivery that is ready joins one landing: HangarAI waits for those arriving (15 seconds without a new one, a minute at most), merges them all, up to 16, runs the check once, and has each one reviewed at the same time, every reviewer in a folder of its own. A red check and requested changes reach the worker together, in one message. A branch you merged by hand counts as landed. A delivery that waits for your go is checked and reviewed with the others, without holding them back, and lands on your go without a second review. Measured with ten ready deliveries, a 30-second check and 20-second reviews: 65 seconds, where one after the other took 500.
- **Delivery speed modes**, per workspace, under **Delivery speed** in its orchestration settings and in the new-workspace dialog:
  - **Balanced**, the default: a delivery lands as soon as it merges, then its check runs; a red check reverts it with a new commit (never a reset) and sends it back to its worker with the output. Only a risky change is reviewed before it lands (licensing, billing, sign-in, secrets, migrations, release scripts, CI, a deleted file, or more than 400 lines; the paths and the threshold can be changed).
  - **Fast**: workers share the workspace's folder, a delivery is done the moment it is submitted, and the check runs after it; a red check opens a fix task with its output, and nothing is reverted.
  - **Safe**: checked and reviewed before every landing, as before 3.0.
  - Delivery cards and the conversation say what the mode did: *Landed · checking…*, *Reverted after a red check*, *Review skipped · low risk*, *Fix task opened*.
- **The merge train.** Deliveries that touch the same files land in the order they came. When two of them only add lines at the same place, both are kept instead of one going back. A check that fails, then passes when run again, is retried once (never for a test the change touched), and the task's card says so.
- **Smart file scopes.** A new task gets a comment listing the files it should own, found in the code from its title and description. Files that every task adds to (a registry, a dictionary) can be named in `.hangar/shared`: tasks may add to them at the same time. A worker may make a mechanical one-line fix outside its files; it lists it, and the reviewer checks it is only that.
- **No task waits for another to land.** A task whose dependencies are delivered but not landed yet starts at once, on a worktree with their branches merged in; when they land, HangarAI brings the target branch into its own and tells its worker, which cannot deliver before them. The orchestrator hears at once when a task can start, and lands in batches.
- **About 12 % fewer tokens for the same work, and about 17 % less spent on models** (estimated on 15 days of real orchestration sessions). HangarAI's tools answer agents briefly: writing to the board returns a short receipt instead of the whole task with every comment, `board_list_tasks` returns a summary of the open tasks (`board_get_task` gives one in full), and workers start without the tools they never use: about 6 % fewer tokens on their own, where these answers reached 2.6 MB. The orchestrator now picks each worker's model and effort for its task: Haiku for mechanical edits, Sonnet for routine work, Opus for design and the hard parts, Fable for the hardest, with effort *high* by default (about 18 % fewer tokens written than *xhigh*) and *xhigh* for risky work, one step up on a retry. A workspace can fix either in its orchestration settings (*Auto* by default), and each worker shows what it runs, as in « Sonnet · high ».
- **40 panes per workspace** (it was 16), and up to 100 under *Panes per workspace* in its orchestration settings: past 16, a new pane also needs 1.5 GB of free memory and processors that are not saturated.
- **Night shift.** **Night shift › Plan a night…**, in a workspace's orchestration panel: leave it objectives, a delivery mode for the night (Fast or Balanced) and a budget (a stop time, points of your plan's weekly window, HangarAI credits), and the orchestrator works through them while you sleep. **Plan a night…** is in the command palette too, and the **Tonight** tile shows the night under way. The budget in points is read on each account the night's agents run on, named in the sheet (« not measured yet » until its agent first reports), never on the plan of whichever session reported last. Nothing is pushed, released, published or deployed during the night, and questions are parked for you instead of waited on. A **morning report** waits as a card at the top right of the window, and on your phone: what landed, the pictures the deliveries made, what waits for you, what did not make it and why, and what the night spent.
- **Replays.** **Export replay…**, in a workspace's menu, turns a period of its board (the last session, 24 hours, 7 days, or everything) into a page: the board moving, deliveries landing with their summaries, the agents and the time it took, with a timeline to scrub, in light and dark, on a phone too. Preview it and choose what goes: secrets, e-mail addresses, paths outside the repository and names are taken out before it leaves, and no file's contents ever go. It is published as a share link, which carries your referral code.
- **Accounts switch before a limit** (Settings › Accounts, off by default): a pane or a worker about to start on an account past 80, 90 or 95 % of its window starts on the account with the most room left. An agent stopped at its limit gets a card in Needs you to restart on another account, on the same conversation.
- **“Waiting for you” means a real dialog.** A pane shows as waiting, in the navigator, in Needs you, on Hangar Mobile and in the orchestrator's messages, only while its agent's dialog is up: the card and the message say what it asks, and the wait is taken back when the dialog closes, however it closed. A permission an agent refused by itself after its two minutes used to leave its pane waiting through the long command that followed, and the orchestrator was told at the two-minute mark.
- **Delivery events that tell the truth.** A branch you merged by hand while its delivery was queued or being checked ends the check and the review and is recorded as landed (HangarAI looks every 10 seconds), and a done task stays done. A check whose machine is gone (no build machine, a lost connection, a full disk, no network) is run again by itself after 1 minute, then 3, then every 10, with the worker told nothing; the orchestrator hears it once when the machine stays down and once when it is back. A main that fails the same check without the branch is not blamed on it: the worker is told once that it has nothing to fix, the orchestrator once per commit of the target, and the delivery is tried again every minute until the target moves. One fact is one event: the same event of a batch is told once with its count, and a session that ended after its task moved on is not announced.

#### The app

- **One view, in tiles.** A workspace is one view of tiles you place as you like: the conversation with the orchestrator, **Agents** (who does what now, one row each), **Needs you**, **Machines**, **Deliveries** step by step, **Cloud** runs, the **Board**, **Activity**, **Changed files**, **Tonight**, **Shared** links, **Accounts & credits**, **Memory**, **Statistics**, previews, kits, the 3D view and the terminals themselves. Two layouts come with it as tabs in the bar, **Chat + team** (the conversation first, the team beside it) and **Terminals** (every terminal in a grid); **+ Tile** (or a drag from its menu onto the grid) adds one where you drop it, **Arrange** moves and resizes them (each kind keeps a smallest size), and your own layouts get a name. The Board, Memory and Statistics open over the grid from the bar (`Ctrl`+`Alt`+`B`, `K`, `U`, `Cmd` on macOS; `Esc` goes back) and **Keep in this layout** keeps them. Tiles arrive by themselves when there is something to see: a question in Needs you, a cloud run started by you or an agent, a machine. Closing a terminal hides it without stopping its agent; the status bar counts them (« 8 tiles · 2 terminals hidden ») and brings one back with a click. In a small window the tiles stack, then become tabs, and the layout comes back intact when the window grows. Images, 3D models and kits shown by agents are cards in the conversation and open as a tile. **Needs you** says one count everywhere. A workspace from 2.x keeps its grid as **Terminals**; one that was in the chat view opens on **Chat + team**, its terminals hidden as the chat view hid them.
- **One HangarAI at a time, running in the background.** Opening HangarAI again brings back the window that is open, instead of starting a second copy. Closing the window leaves HangarAI running: agents keep coding, deliveries land and Hangar Mobile stays connected. The tray icon shows how many agents work or wait for you, with *Show HangarAI* and *Quit HangarAI*; **Quit HangarAI** (`Ctrl`+`Shift`+`Q`, `Cmd`+`Q` on macOS) asks first while agents are in the middle of a task. The window comes back at its size and place. Settings › General turns it off.
- **Clickable file paths in the terminal.** A path a pane prints (`src/a.rs:12:5`, `a.ts(12,5)`, a Python traceback's line, a `git status` line) is underlined under the pointer when the file is there. A click opens it in the workspace's preview at that line, marked; a folder opens in the file manager; Alt-click (Option on macOS), or *Open with VS Code* in its menu, opens it in VS Code at the line.
- **Drop anything on a chat.** In the conversation, drop a folder, a file, an image or several at once on the conversation: each becomes a chip in the composer, where the caret was, and goes in the message as its absolute path, the way the agent opens it. While a question or a plan waits for you, the path goes into its answer. In your messages, the paths that exist are links: a file opens in the preview at its line, a folder in the file manager.
- **Paste into a chat too.** Ctrl+V (Cmd+V on macOS) in the composer pastes a screenshot or an image copied from a browser: it shows as a thumbnail above your words, as in a chat app (× to take it out, a click opens it in the preview), is saved in the workspace's `.hangar/images/pasted/` and goes in the message as its path, the way the agent opens it; once sent, your message shows it in its bubble. A picture you drop shows the same way. Files copied in a file manager become a chip each, as if dropped, and text pastes as before; a question's or a plan's answer takes them too.
- **Previews you act from.** A preview's header has what you came for: copy the image (as a PNG) or the text, copy the path, show it in its folder, and under **More actions** open it with its app or in VS Code (at its line), save a copy, rotate an image, ask the orchestrator to edit it (its credits asked as usual), reload, share or close; a right click on the content opens the same menu. While it has the focus, `Ctrl`+`C` copies the image or the text, `Ctrl`+`Shift`+`C` (or `Ctrl`+`Alt`+`C`) the path, `Ctrl`+`S` (or `Ctrl`+`Alt`+`S`) saves a copy (`Cmd` on macOS) and `Space` or `Esc` puts a zoomed preview back; each says « Copied » or why not. A file of a workspace on a node copies its path and its picture but is not opened here. In the conversation, images an agent made and pictures you pasted have the same actions under the pointer and on a right click.
- **Long text never overlaps.** Menus, dropdowns, lists, tabs, chips, the palette, dialogs' titles and toasts keep a label on one line, cut with an ellipsis (in the middle where the end matters, as for a worktree's name), with the whole text on hover; the values on the right are never pushed aside.
- **Statistics by project.** Sessions run in a project's worktrees count under the project, even once the worktree is gone, on Hangar Mobile too.
- **Accounts in the status bar.** With several Claude Code or Codex accounts in use, the status bar shows one chip for each, with how many panes run on it; click one to see where its agents are and to switch.
- **Agents on any node.** A node's card, in Settings › Nodes and in the navigator, installs Claude Code and Codex there from their official installers (each step and the installer's last line shown, then *Installed · version x*), then signs them in: a pane on the node runs the agent's own sign-in, which you finish on the provider's page in your browser (Claude Code's `claude auth login`; Codex's device code, since its browser callback would reach the node, not this computer), or **Use an API key** of your own, which goes through the SSH tunnel to the node alone and stays there, never sent to HangarAI's servers. It works the same on a hosted node and on a machine of your own, and on a hosted node the first worker pane opens the sign-in by itself. HangarAI never reads or copies an agent's login.
- **Settings use the window's room.** The sheet takes 88 % of the window, from 760×540 to 1280×900, and follows a resize without a jump; its pages lay out for the width they get (the theme grid holds as many cards as fit, 3 at the least and 6 on a laptop, and Mobile, Kits, Accounts, License, Cloud, Discord, Shortcuts and Nodes take the whole width), with a row's label and hint wrapping at a reading width.

#### Security

- **A signed update feed.** HangarAI installs an update only when the feed and the build are both signed with its release key and named for that version, and it never goes back to an older one.
- **HTML previews off the network.** A page or an SVG an agent shows loads nothing from the internet and reaches no server, WebRTC included, until you let that page alone reach it, over HTTPS only.
- **A repository's own git settings run nothing.** HangarAI's git commands turn off a repository's file-system monitor, its clean and smudge filters, its textconv drivers, and its hooks on a status or a diff.
- **Kits from the Library are pinned to a commit**: a tag that moved is refused.
- **Two security audits**, of the app as shipped and of HangarAI's servers, and their follow-ups: the license is checked in more than one place, against a clock that cannot be wound back, on a sealed binary; a share link's files cannot be replaced once published; trials and free licenses cannot use up a hosted feature's monthly capacity before paying customers; the relay and the sign-ins hold up against floods.

#### The site

- **hangarai.dev, redone**: the pricing page (trial, Pro, Max, credits, questions), checkout, your account, Invite & earn with `/r/` links, the affiliates page and its terms, HangarAI Cloud's terms of use, a privacy page that lists each hosted service's providers and what they keep, the share-link viewer, a header and a footer on every page, pages that use the width of large screens, and new captures of the app, light and dark.

### Changed

- **HangarAI's source is private.** 3.0 is proprietary software; versions up to 2.5.1 stay under the MIT license. The public repository keeps the releases, this changelog, the guide, the site and the issue forms.
- Every workspace delivers in **Balanced** mode unless it is set to another: Safe, the only way before 3.0, stays where it is chosen.
- Up to 16 deliveries share one run of the check (it was 4).
- *Pin in place* keeps a pane exactly where it is: pinning or unpinning no longer moves it to an edge of the grid.
- Settings: "Shells only, without an agent" (it named Claude Code alone), and the messages that named Claude Code now name the pane's agent.
- The license is checked online on launch and about once a day (license id, a hash of the machine, a keyed hash of what the system says of the computer, the app version); see the privacy page.
- In French, HangarAI and Hangar Mobile say *tu* everywhere, and name things one way: workspace, panneau, fusionner. In both languages, the Hangar account is the HangarAI account, and the statistics' notes name Codex beside Claude Code.
- Nodes, the Discord connector and pane scopes (Linux) are no longer experimental: always offered, with Settings › Nodes and the Discord bot in Settings › Discord for everyone. Pane scopes are on by default where systemd's user manager runs, with **One systemd scope per pane** in Settings › General to turn them off; Settings › Experimental is gone while nothing is experimental.

### Removed

- The chat view and the Panes view, the Terminals · Board · Memory · Statistics tabs and the inspector: the tiles replace them (the inspector's files are the **Changed files** tile).
- On-device dictation (whisper.cpp), its model downloads, its cleanup pass and its vocabulary, and the option to dictate with your own Groq key: dictation is hosted now.

### Fixed

- Long names wrapped onto a second line inside fixed-height rows and overlapped the next one, in Statistics' project menu and in many other menus and lists.
- With several Claude accounts, the status bar showed only the last one used.

## [2.5.1] - 2026-11-19

### Fixed
- Codex: the reviewer set for a workspace (Settings › Agents, or the workspace's own choice) now reviews its deliveries, with an account of its own agent; it was always Claude Code.
- Codex: accounts follow the agent everywhere. A Codex pane, reviewer or new workspace picks among Codex accounts, and the new-workspace dialog's account picker follows the orchestrator you choose.
- Codex panes: the context gauge, the restart after an MCP change and Hangar Mobile's chat of a node's pane work for Codex too; a cloned repository gets your default orchestrator agent; the Discord presence no longer names Claude Code alone.

## [2.5.0] - 2026-11-19

### Added
- Codex (beta). HangarAI runs OpenAI's Codex CLI beside Claude Code, or on its own: Hangar works with either one installed, or both.
  - Setup: both agents on the first-run sheet, installed by their official installers and signed in from a pane; either one is enough.
  - Workspaces: choose the orchestrator's agent and the workers' agent when you create one; mixed teams work (a Claude Code orchestrator with Codex workers, or the reverse). Settings › Agents sets the defaults, Codex's launch command and models.
  - Panes: a Codex pane shows whether it works, waits or needs you, like a Claude Code pane. Hangar attaches to the pane's Codex server to follow its turns and answer its approvals (Settings › Attach Hangar to Codex, on by default); without it, the pane still runs, with less to show.
  - Board, memory, kits and Hangar's tools reach Codex too, through `-c` options: your `~/.codex/config.toml` is never edited. The playbook goes to `AGENTS.md` as well as `CLAUDE.md`.
  - Accounts: Codex accounts beside Claude ones, each in a home of its own, chosen per pane, workspace or folder.
  - Images: what Codex generates shows in the chat view, on Hangar Mobile and in Discord.
  - Delegate: an agent hands a sub-task to a Codex or Claude Code child that runs with no terminal, in its folder or a worktree of its own, and gets back its answer, files, images and branch (`delegate`, `delegation_status`, `delegation_cancel`, `delegation_list`).
  - Also: sessions resume and sleep for either agent, Hangar Mobile answers Codex's questions, Discord follows Codex panes, nodes run Codex, statistics per agent with OpenAI's prices, the reviewer can be Codex.
  - Codex is in beta: report what goes wrong with the « Codex bug » issue form.

### Changed
- README, guide and site speak of both agents.

## [2.4.6] - 2026-11-19

### Fixed

- hangarai.dev also shows in the title bar with the navigator hidden and in workspace windows.

## [2.4.5] - 2026-11-19

### Added

- Pin a pane: a pinned pane keeps its edge and its size while workers and other panes open and close, and presets never close it. Pin and unpin it from the pane's menu or the command palette; a pin marks it in its header.
- Window opacity, for every theme (Linux): Settings › Appearance › Window opacity lets the desktop show through the window, the panes and the terminals, while each theme keeps its own colours. Blur is the desktop's: KDE does it, GNOME with an extension such as Blur my Shell.

### Changed

- HangarAI: the new name, with hangarai.dev always shown in the title bar (a click opens the site). The program, its folders and its settings keep their names, so nothing moves on an upgrade.
- Contact: contact@hangarai.dev, for questions about your data, conduct reports and security reports by email.

### Fixed

- Discord: a message written in a channel while its agent was busy could lose its place in the queue, and its ⚡ then did nothing.

## [2.4.4] - 2026-11-19

### Changed

- Hangar's address is now hangarai.dev: the website at https://hangarai.dev and the optional Hangar account's server at https://account.hangarai.dev. The old addresses (hangar.simnjs.fr, hangar-account.simnjs.fr) keep working and lead to the new ones.

### Fixed

- Typing in a text field while a pane was selected went to the pane: the "Send the same instruction to all panes" bar, the terminal's search, renaming in the navigator and adding a card on the board lost the keyboard to the terminal, often after the first letter. A field that takes the keyboard now has it until you click a pane again, everywhere in the app.

## [2.4.3] - 2026-11-18

### Added

- Coloured badges for Claude accounts (Settings › Accounts, off by default): a colour and an icon per account, shown on the toolbar's account button and its menu, and wherever an account other than the default is named. Contributed by SimonB44470.
- A Claude account per navigator folder: workspaces without an account of their own run on their folder's, and follow it when moved; an explicit choice, "Default" included, still wins. A "+" on a folder's heading creates a workspace in it. Contributed by SimonB44470.
- With two Claude accounts or more, the navigator's plan block shows each account's own limits and next reset. Contributed by SimonB44470.

### Changed

- Choosing another Claude account no longer asks to restart running agents: each one moves by itself, on the same conversation, as soon as Hangar knows it has nothing to lose (its turn over, no draft in its prompt, no command of its own still running); meanwhile its header shows where it is going and why it waits. "Ask before moving running agents" in Settings › Accounts brings back the question. Contributed by SimonB44470.

## [2.4.2] - 2026-11-18

### Changed

- A quieter orchestrator on big projects. A worker waiting on a job of its own (a build, a benchmark, a CI run it watches) is no longer reported idle every ten minutes. A worker that is really idle is reported once, then again only after it did something or half an hour later. News that can wait (plain comments, idle workers) comes as one digest every five minutes at most, while deliveries, questions, a worker waiting for you or a crash still arrive at once.

### Fixed

- Enabling a kit works again. With its activation sheet open over a workspace's Kits dialog or over Settings, the click on "Accept and enable" went to the dialog underneath: the sheet closed and nothing was enabled, and in Settings the click could flip another switch. Only the dialog in front now takes the mouse and the keyboard.

## [2.4.1] - 2026-11-17

### Fixed

- The Hangar account reaches its server at `hangar-account.simnjs.fr`: 2.4.0 pointed at an address that does not exist, so signing in failed.

## [2.4.0] - 2026-11-17

### Added

- **An optional Hangar account.** Settings › Hangar account (« Compte Hangar ») lets you sign in with GitHub, in your system browser, to link your machines. It is off by default: Hangar works as before without one, never calls the account server until you sign in, and still sends no telemetry. Signed in, the page shows who you are, your machines (name, system, last seen, « This PC »; rename one, remove another with a confirmation), Sign out, and Delete my account (type the word to confirm; it deletes everything on the server, then signs this PC out). A « Sync statistics » switch, off until you turn it on, sends totals per machine, day, model and project name (tokens, active time and cost; never a path, a prompt or any code) at once, every 15 minutes, when you open Statistics, and on « Sync now », without ever waiting on the network in the window. A day's totals already sent are never replaced by lower ones, so Claude Code cleaning old transcripts does not shrink your history. Errors read as plain sentences (server unreachable, signed out from another machine). The token stays in your system's keychain. The details are on the [privacy page](https://hangar.simnjs.fr/privacy.html) and in docs/native/cloud.md.
- **Statistics of all your machines.** Once synced, the Statistics page has a « This PC / All machines » switch. This PC is the page as it was; All machines shows the merged totals of every machine of the account by machine, day, project and model, for the period chosen, with when the account was last synced.
- **The account server, ready to run with Docker.** `ghcr.io/hangar-ai/hangar-cloud`, built, checked and published by CI, and `crates/hangar-cloud/compose.yaml` to run it behind a reverse proxy or through a Cloudflare Tunnel (`docker compose up -d`), with its health check, a read-only root and no capability. For whoever hosts it; docs/native/cloud.md, « Deploy », walks through it.
- **Hangar on NixOS.** A Nix flake at the root of the repository: `nix run github:hangar-ai/hangar` builds and starts Hangar with its webview helper and Hangar Mobile, its package goes in a NixOS or Home Manager config, and `nix develop` opens a shell with everything a build needs. On Nix, the MCP configs and the login item name the `hangar-ai` on your `PATH`, not the store path, so they survive upgrades and garbage collections. Nodes are not in the package yet. Contributed by Kisakay.
  - On NixOS, the **Set up Claude Code** sheet offers Claude Code from nixpkgs instead of Anthropic's installer, whose `claude` does not start there without nix-ld: the line for the configuration (`environment.systemPackages = [ pkgs.claude-code ];`) and a `nix profile install` command, to copy, with what claude-code being unfree asks for, then **Check again**. The installer stays behind **Use Anthropic's installer anyway**.

### Changed

- **Discord: a thread per worker, shared files, and ⚡ to send now.** A worker's thread opens by itself when it starts (its « Ouvrir le fil » button stays only if Discord refuses it), a file an agent shares (`phone_share`) is posted in its thread, or the workspace's channel for the orchestrator (one too big for Discord becomes a line saying it is on the phone), and a message waiting for the agent's turn to end gets a ⚡ that you click to stop the turn and send it at once, as « Send now » does in the chat view.
- **The viewers open any file on the computer.** Images, PDFs, Markdown, code, video, web pages and 3D models shown by an agent (`preview_show`, `model_show`) or by you no longer have to be inside the workspace's folders, and a workspace whose folder is not a real path shows files too. What the viewer pages may load stays limited to what was shown (and, for a 3D model, the textures and buffers beside it): a page cannot read the rest of the disk.

## [2.3.0] - 2026-11-13

### Added

- **A Hangar with no window, on a server.** A node runs workspaces of its own now, with no desktop connected: their state is kept on the node, their panes start there with Claude Code, and their agents' requests (workers, deliveries, the board) are answered there, so they keep working with your computer off. Make one with `hangar-node workspace add <folder>` on the server (or from Discord); `hangar-node workspaces` lists them, and a connected desktop lists them in its navigator among the others, opens them with their terminals live, and changes them there (a rename, a pane opened, closed or restarted, a preset, a deletion: each is sent to the node, which stays their owner; while the node is offline they can be looked at, not changed). Merge on one of their deliveries and Stop workers are carried out by the node. A workspace made from the desktop with « Runs on » set to a node is the node's own from the start, and one made before, which still stops with your computer, has « Move to {machine} » in its menu in the navigator: its agents are restarted on the machine on their sessions. If the machine's answer is lost on the way, the workspace waits as it is and the machine's own list settles it at the next read; and a workspace a machine came back without after a restart is kept in the list, marked, with « Run on {machine} again » in its menu. `hangar-node service install` makes the node a systemd service that starts on its own.
- **The same repository on several machines (nodes, experimental).** Hangar knows a repository by its `origin`, however the address is spelled. When one is open on this computer and on a node, each of its rows says so in the navigator (a faint mark on a local row, « also on… » in the tooltip of a machine's mark), and its menu starts with « Also on », which goes to the other checkouts. « Open on another machine… » in that menu opens the new-workspace dialog ready to clone it where it is not yet. In that dialog « Runs on » is there for a clone too, and lists every machine (one that is offline or switched off is greyed, with its state; from four machines on, they are a menu): the node clones with its own keys, into a folder you pick there (`projects` in its home by default), shows its progress, and runs the workspace itself. If it cannot clone, git's own words are shown.
- **Hangar on Discord (experimental).** Control your Hangar from a Discord server of your own, as from the phone: a channel per workspace (those on your nodes included), its orchestrator's turns as cards and its answers under its name, each agent with its own face; what you write in the channel goes to the orchestrator, waits in its queue while it works (⏳, then ✅ when it is taken), and an edit or a delete changes what waits; permissions, questions and plans as cards answered with buttons; workers announced with « Ouvrir le fil », which opens a thread with their work since they started, then live, where you can talk to them; deliveries with Land and Send back; a pinned dashboard; `/stop`, `/model`, `/effort`, `/plan`, `/status` and `/workspace`. Only the account you pair with a code is heard. Turn it on in Settings › Experimental, then give the bot's token and your server's ID in Settings › Discord; it runs while Hangar runs, window closed or not. The same bot serves the workspaces your nodes run by themselves: the desktop hands it to them, and they keep answering in their own channels with your computer off. A node with no desktop can have a bot of its own (`hangar-node discord setup --guild <id>`).
- **Answers reach Claude Code as answers.** A permission allowed or denied, or a question answered, in the chat view or on the phone now goes to Claude Code as the decision itself, through the hangar-bridge mod, as an Agent SDK host's would: no keys typed into the dialog, no screen read back. The dialog stays up in the terminal meanwhile and either place can answer; a pane on a node, or a Claude Code without the mod, is still answered with keys.
- **Stop in the chat view.** While the orchestrator works, a button next to Send stops its turn, as Escape does in the terminal.
- **Model, effort and plan mode from the chat view's input.** Next to the input, the orchestrator's model, its effort level and Plan: a click shows the choices, and the choice reaches its Claude Code as `/model`, `/effort` or `/plan`, at once when it is idle, at the end of its turn otherwise. The hangar-bridge mod reports what the session runs with as it changes, so the input shows it. As in the terminal, `/model` and `/effort` also become the defaults of new sessions.
- **Files Claude names are links, in the chat view.** A file of the workspace that Claude names in a message (`src/search/index.ts:42` in code, or a link to a relative path) opens in the workspace's preview, beside the conversation, with a click. Only names that are files there become links; code blocks are left alone.
- **A queue you can see and change, in the chat view.** What you send while the orchestrator works now waits in its queue instead of being typed into the turn, and shows at the top of the input: each message can be edited (back into the input) or removed while it waits, and Send now stops the turn so the first one goes at once. Claude takes them when its turn ends. Through the hangar-bridge mod; without it, messages are typed as before.
- **What a delivery brought, in the chat view.** A delivery that landed or waits to land shows how many files it changed and their +/− on its line; a click lists the files, and a file opens the Changes window on exactly what that delivery brought (its commits, not the working tree). The orchestrator is told the same range (`git diff <base>..<merge>`), so it can look at the changes too.
- **Plans in the chat view.** When the orchestrator presents a plan (plan mode), the chat view shows it whole, formatted, in a card with Approve the plan and Keep planning; a field says what to change in it, and Claude plans again with those words, the turn going on. Through the hangar-bridge mod.

### Changed

- **Several machines, at a glance (nodes, experimental).** A machine goes by one icon, its kind's (server, cloud, laptop, desktop, mini PC, tower), chosen when it is added and changed from its row. In the navigator a workspace on a node wears its machine's icon, the filter finds it by the machine's name or narrows the list to one machine, and a line at the foot counts the machines and, in the warning colour, those that are offline (those switched off are in its tooltip); the palette and the new-workspace dialog name machines the same way. A node now connects as Hangar starts and reconnects for as long as it is down (2 s doubling to a minute, at once when you come back to the window or press Retry now; an attempt that hangs is given up after a deadline). A first connection that takes a while, an install for instance, reads as connecting, not offline. A link that drops says nothing for 5 seconds; then its rows step back with an unplugged mark, and its panes say why with Retry now and, after 20 seconds, Switch off, which hides the machine's workspaces, in Needs-you too, until it is switched on again in Settings › Nodes. That page lists the machines as rows, this computer first, each with a switch and a menu (Retry now, Rename, Remove from this computer). A failed connection reads as one short sentence (« Offline · retrying in 8 s: unreachable over SSH (Connection timed out) »), with what ssh said under it.
- **The chat view reads like a chat.** Claude's words sit open on the page, your messages in rounded bubbles on the right; its calls are quiet lines with an icon for what they do (a command, an edit, a read, a search, a task), folded in runs as before. While it works, a line at the bottom says so and for how long, and the header gives the model and how full its context is. The input is one rounded composer; while a question or a permission waits, its card takes the composer's place, with only the question being asked (no recap of the answers given before: Back goes to them).
- **The orchestrator's exchanges with its workers, short and in your language.** What Hangar tells the orchestrator about the workspace shows as one line per thing that happened, with an icon and a colour for what it means (Ada delivered “Search index”: merged, task done; Bo started on “Filters”; Cy has been waiting for you for 4 min) instead of the block written for the model. A worker's message to the orchestrator and the orchestrator's message to a worker each show as one line with their first words (Bo: “…”, To Bo: “…”), the whole message a click away, and a message is never folded into a run of calls. The state of the work handed to a conversation that starts is one line too.

### Fixed

- **A long question could not be answered from the chat view or the phone.** Claude Code now draws a bar down the left of a question's lines; when a question wrapped over several lines, Hangar no longer recognised it on the screen and refused to type the answer ("The question is not on the PC's screen…"), so the first, short questions went through and the longer ones that followed did not. The answer now goes through the mod with no screen involved, the screen check ignores the bar where keys are still typed, and a terminal the chat view hides starts at 160×50 instead of 80×24, so a long question is drawn whole.

## [2.2.1] - 2026-11-08

### Added

- **A delivery's progress is on its worker's pane.** A chip says where it stands (queued, merging, checking with the time it has taken, in a batch of N, reviewing, landing) and then how it ended; the chat view's tasks panel shows the same on the task.
- **Deliveries are checked together.** When several wait, Hangar merges them on the branch and runs the check once for all of them; if it fails, it splits them to find the one that breaks, and only that one is handed back. The branch stays exactly as safe: nothing lands on a tree the check did not pass. With five deliveries waiting and a one-minute check: 73 s instead of 315 s when all pass. The batch size is in the orchestration settings (4 by default).
- **Claude Code is set up with Hangar.** When it is missing, a first screen installs it in one click with Anthropic's official installer (its output shown as it runs), then opens Claude Code to sign in; a workspace cannot be created without it, unless you choose to use Hangar with shells only (Settings › General).

### Changed

- Releases ship Windows and Linux. macOS is no longer built for download: 2.2.0 is the last macOS release, and the README says how to build Hangar on a Mac.

## [2.2.0] - 2026-11-07

### Added

- **A new workspace starts with its orchestrator.** Creating a workspace now gives one Claude Code pane that is the orchestrator, ready to plan and hand work to workers; the layouts of several panes and shells are under More options. In a folder without git it still orchestrates, its workers sharing the folder (git init gives them worktrees).
- **Chat view (experimental).** Turn it on in Settings › Experiments, then switch any workspace between its panes and a chat with its orchestrator. Claude Code keeps running in the panes behind it; a side panel lists the board's tasks with the progress their agents report, and kits, previews and the 3D view open beside the chat as they do in the grid.
- Agents can report how far along a task is (`board_report_progress`, over MCP and HTTP): a percentage and a short note, shown with the task. Workers are asked to report at each real step.
- **Nodes (experimental).** A headless Hangar, `hangar-node`, runs on a Linux machine you reach over SSH: add it in Settings › Nodes and Hangar installs it, starts it and keeps a tunnel to it. A workspace can then live on the node: its terminals, Claude Code and board run there, and the window shows them like a local workspace's, catching up on what happened while it was away. Its memory, transcripts, chat view and orchestration work too: the orchestrator and its workers run on the node, in worktrees there, while the connected window drives them. A local workspace can also send a worker to a node (`worker_spawn` with `node`), for a heavy build or a GPU box: the worker runs there and its branch comes back to be checked, reviewed and landed here. Design in `docs/native/nodes.md`.
- **Pane scopes (experimental, Linux).** Each pane can run in a systemd scope of its own, so when memory runs out the system stops the pane that uses it (a build, say) rather than Hangar and every agent with it.

### Changed

- Released builds use link-time optimization across the whole program, rarely used code is built for size and unused image codecs are left out: Hangar's executable is 44 % smaller (119 → 67 MB, hangar-node 17 → 10 MB) and parses terminal output a few percent faster. TIFF and ICO images no longer show in kit tabs.
- Previews are lighter: a large image is shrunk when it is decoded and read in full only when zoomed (an 8000×6000 PNG: +554 → +143 MB), a Markdown page no longer colours its whole source unseen (38 % → 4 % CPU) and parses off the interface thread, memory freed by a closed preview goes back to the system, and the web view helper stops a minute after its last window.
- At rest, the panes no longer wake the window several times a second: about once every 20 s (43 wake-ups a minute → 3 with a shell pane), and Hangar's CPU at rest falls by about 40 %.
- Drawing costs half as much or less: a busy terminal 0.97 → 0.48 ms a frame, 16 printing panes 2.7 → 1.3 ms, scrolling back through history 4 → 1 ms (a scroll plans only the rows that came into sight), selection 3.9 → 0.9 ms, the statistics view 1.8 → 0.95 ms. The terminals' lock no longer makes the window wait behind panes that print.
- Output from panes that are not on screen no longer makes Hangar rebuild and redraw its window 60 times a second: with eight agents printing, Hangar's CPU falls from 8.1 % to 2.9 % in the chat view and from 6.4 % to 5.8 % in the panes view.
- On relaunch, the next wave of resumed sessions starts as soon as the previous one is under way (its agent printed and went quiet) instead of a fixed 2 s later: eight sessions in the chat view are all back in 3.2 s instead of 9.1 s. Agents that keep printing still hold the next wave as long as before.
- The login shell's PATH is read from the first instant of the launch, beside the window's start: with a slow shell profile (0.35 s), the first prompt comes about 90 ms sooner.
- In the chat view, the hidden terminals start their shells at once instead of waiting for a size they never get: a workspace in the chat view is ready in 0.38 s instead of 0.66 s.
- On Linux with a Vulkan GPU, Hangar no longer also sets up an OpenGL context it never draws with: about 27 MB less memory at rest and a faster start (0.40 → 0.36 s with 4 shells). A `WGPU_BACKEND` you set yourself is kept, and the one Hangar sets does not reach your panes' shells.
- Terminal output is parsed faster (+6 % on a listing, +8 % on an agent's interface): the scan for title and link sequences skips plain text in one search.

### Fixed

- Chat view: a question with several parts is asked one part at a time, as Claude Code asks it (1/3, Back, Next, then your answers and Send), its card scrolls instead of overflowing, and a missing answer or a refusal is said beside the buttons instead of Send doing nothing. Runs of tool calls fold into one quiet line ("3 commands, 2 tasks created · 1 failed") that opens on a click.
- Windows: a pane whose folder is longer than 258 characters starts its shell (in the folder's short 8.3 name) instead of failing with "The directory name is invalid".
- Windows: Hangar starts on a Windows without the Visual C++ Redistributable. The installer now puts the runtime's three DLLs beside the executables; before, they exited at once (STATUS_DLL_NOT_FOUND) on a fresh install.
- A Hangar started in a folder inside a worktree no longer removes other workers' worktrees: the sweep of orphan worktrees at start only removes the worktree of a task that is done, and never one a live process works in; landing a delivery and stopping a worker keep a folder another pane or process uses.
- Kits, Windows: removing a kit's clone folder retries for a moment when a git process that was just stopped still holds it, instead of failing.
- Tests remove the temporary folders they create, even when they fail; a full test run used to leave thousands behind in `/tmp`.

## [2.1.0] - 2026-11-06

### Changed

- On relaunch, the Claude panes on screen resume their conversations first; the others follow two at a time, or at once when they come into view, so a workspace full of big sessions no longer starts every Claude Code at the same moment.
- A change to the app's state no longer copies and compares the whole state on every action, the state snapshot is rebuilt only when it differs instead of on each keystroke, and saves are serialised off the UI thread.
- Less work while agents run: the session hunt lists transcript file names and dates only instead of reading the head of every transcript every few seconds, skips panes the mod already tracks, and settles to a 5 s cadence for resumed panes; the session picker reads at most 256 KB of each transcript and caches its labels.
- The context gauge and the phone's chat read transcripts without a lock shared by every pane, stream a transcript instead of loading it whole, and find it under `CLAUDE_CONFIG_DIR` and account profiles too.
- Kits ask again for your consent when a script, hook or MCP server file they run has changed since you allowed them, not only when their command lines change. An update is checked in full (manifest, permissions) before it replaces the installed kit, and a kit installed from a tag notices when that tag later points elsewhere.
- Each pane gets an API token of its own, kept across a restart or a hibernation and revoked when the pane is closed; a pane's token fixes who the caller is, so an agent can no longer act as another pane or as you through the local API. `~/.hangar/endpoint.json` now advertises a token for outside callers instead of Hangar's own.
- The terminal's scrollback is capped so a pane cannot hold tens of MB of history; box-drawing characters are cached instead of being redrawn from scratch every frame, and an open search no longer rescans the whole history on every chunk of output.
- The command palette filters once per keystroke instead of on every redraw; a native kit tab compiles off the UI thread; pictures, highlighted code blocks and the stats page are computed in the background.

### Security

- `~/.hangar/endpoint.json`, which holds the API token, is readable by its owner only (0600, folder 0700); trusting a folder keeps `~/.claude.json`'s permissions.
- The updater keeps its download in a private folder and checks the signature again right before installing, so the file cannot be swapped in between.
- Phone actions (send, allow, answer, land) carry a key and run once even when the phone retries; allowing a permission from the phone applies only to the prompt the phone showed.
- Voice models are pinned and checked against their SHA-256 before use.
- A kit tab cannot make Hangar hold unbounded logs, images or messages, or block the window with a long chain of updates.

### Fixed

- Hangar no longer replaces a `CLAUDE.md`, `.gitignore`, `board.json`, `.mcp.json` or `~/.claude/settings.json` it cannot read or parse (not UTF-8, unreadable, invalid JSON): it leaves the file alone and says so; a corrupt board is kept aside as `board.json.corrupt.<time>`. Every state, board and config file is written atomically and flushed to disk.
- One pane whose program stops reading its input no longer freezes typing in every other pane; panes get a short grace to exit on quit, so Claude Code can finish writing its transcript; on Windows, what a pane started no longer outlives Hangar.
- A `postCreate` hook that leaves a process running no longer blocks a worker's start; Hangar's git commands never wait for a password and stop after 2 minutes on the network; the delivery reads a task's `owns` exactly as the board does; check folders of deleted workspaces are removed.
- Copied history in resumed or forked sessions is counted once in the stats.
- A 3D model loader that crashes, or a snapshot of a model in a sleeping workspace, no longer leaves the 3D view waiting forever; a zoomed SVG is no longer rasterised at full zoom size.
- The terminal redraws the rows it scrolls to after a search, and clearing it during a synchronized update works.
- A field focused under a dialog loses the keyboard; menus taller than the window scroll to their last entry; the palette no longer jumps to the row under a resting pointer.
- The update banner no longer stays on "restarting" after a failed install; "Download again" repairs a damaged voice model; a voice transcription that cannot start no longer leaves the button stuck; Discord presence no longer lags one update behind.
- Deleting a workspace removes its delivery check worktree, and a last save that fails at quit is written to `hangar.log`.
- A worker is never given a block of ports one of which another process already listens on.
- Clicking a notification or a Needs-you item closes the settings cleanly, and a voice model download keeps showing its progress when you switch settings categories.
- A dialog opened over another one, such as a kit's activation dialog over the settings, is drawn opaque and on top: the settings' scrolled content no longer shows through it.
- A relaunch no longer leaves an agent pane on a bare prompt: its command is typed on an emptied line (keys that reached the focused pane first are no longer glued to its front) once the shell has printed its prompt and gone quiet (never before the launch delay, at most 3 s after it), is typed again if the shell runs nothing after it, and a launch lost twice is logged. A saved conversation whose transcript is not found once is started fresh but kept, and forgotten only after a second miss in a row.
- A pane started at launch before Hangar had finished looking for the machine's shells no longer runs the default shell instead of the one it was given: its shell starts once detection has answered.

## [2.0.0] - 2026-11-05

Hangar, rewritten as a native app. It keeps your data, your workspaces and the features
around the terminals, and needs a fraction of the memory and processor it did.

- **Native, in Rust.** The whole app is Rust now, drawn with iced, and runs no web view: 1.x
  was a web page in a Tauri window. The terminals are Hangar's own, on alacritty_terminal.
  - What only a browser engine can show, a kit's HTML tab, a PDF, a web page, a video,
    opens in a separate helper program, `hangar-webview`, in a window of its own.
  - Measured on the same Linux machine, 1.12.1 against 2.0.0:
    - starting cold: 0.96 s, now 0.57 s;
    - memory at rest: 238 MiB, now 166 MiB;
    - 16 terminals printing non-stop for 30 s: 8.1 GiB of memory, now 249 MiB, and 4.4
      processor cores, now 2.4;
    - the echo of text sent to a pane while 15 others print: none within a minute, now
      2.5 ms.
- **Claude Code only.** Hangar runs Claude Code and plain shells. Codex, Gemini CLI and
  OpenCode are gone, with their launch, session resume, statistics, prices and settings.
  - A pane saved with one of them opens as a shell in the same folder, says so once in its
    bar, and is saved as a shell.
  - The MCP panel writes Claude Code's `.mcp.json` only, and the agents' playbook goes to
    `CLAUDE.md` only. What earlier versions wrote for Codex, Gemini CLI, Cursor, VS Code or
    in `AGENTS.md` is left as it is.
  - The statistics read Claude Code's transcripts; the "By agent" card is gone.
- **Native kit tabs.** A kit's tab can be Rust compiled to WebAssembly. It describes its
  page as widgets that Hangar draws, in your theme and your language, with no browser
  engine, and runs in a sandbox that reaches only what its manifest declares.
  - The **kit SDK** (`hangar-kit-sdk`) has the `Kit` trait, view builders, the calls a tab
    makes, and a harness that runs and tests a tab without Hangar. The contract is in
    [docs/native/kits-native.md](docs/native/kits-native.md).
  - A native tab needs `"schema": 2`. A kit that must still open on 1.x keeps its HTML page
    and adds a `native` module beside it: 1.x opens the page, 2.0 the module.
  - HTML tabs keep working, with the same `window.hangar` API and the same permissions.
  - A compiled tab is kept on disk, so it opens at once after a restart.
- **Same data, same updates.**
  - 2.0 reads the same folders and the same `state.json` as 1.x: workspaces, panes, boards,
    memory, settings, theme, language, shortcuts, kits and Claude accounts are all there. It
    writes the file back in the shape 1.x reads, fields it does not know included.
  - A 1.x install updates to 2.0 through the same update feed, signed with the same key. The
    executable is still `hangar-ai`, so the MCP configurations agents already have keep
    working.
  - Two things 1.x kept in its web view's storage are not carried over: the kit tabs left
    open, and each Claude account's cached usage. Open the tabs again once; the usage comes
    back at its next refresh.
  - A `state.json` that cannot be read is moved aside as it is, never replaced by an empty
    one.
- **Hangar Mobile is unchanged**: the same phone app, the same pairing, the same
  notifications.
- **Hangar draws its own title bar, on Linux and Windows.** Its windows open without the
  system's title bar: the toolbar and the top of the navigator are the bar, in your theme.
  Press on it to move the window, press twice to maximise or restore it, right-press for the
  window menu; the edges resize the window.
  - On Linux, the buttons are the ones your desktop shows, on its side (GNOME's button
    layout), and the corners are rounded as GNOME's own windows are.
  - On Windows, minimize, maximize/restore and close sit on the right as in Windows 11, the
    close button red when you point at it. A window still snaps to the screen's edges, and
    Windows 11 rounds the corners and draws the shadow.
  - **Appearance › Use the system's title bar** brings the system's back, on Linux and
    Windows. macOS keeps its own.
- **A Linux icon shaped like the others.** The dock and the app grid now show Hangar on a
  rounded tile with room around it, the shape GNOME and KDE give their own app icons,
  instead of a square from edge to edge; the packages carry it from 16 to 512 px and as an
  SVG.
- **Images paste into Claude Code.** With an image on the clipboard (a screenshot), Ctrl+V
  in a pane now reaches Claude Code, which attaches the image as it does in any other
  terminal; before, Hangar took the key to paste text and, finding none, did nothing.
- **The arrows and Escape stay in the pane.** The sidebar's filter answered Down and Escape
  wherever they were typed, so the first Down arrow in Claude Code's choices took the
  keyboard to the workspace list; the status bar's line took Alt+Enter the same way, the
  board's new-card field Escape. Those fields now hear keys only while they have the
  keyboard. A click in a pane also always takes the keyboard back from the sidebar.
- **Agents' panes are laid out, not stacked.** A pane an agent opens or closes (through
  Hangar's MCP server, or the orchestrator's workers) no longer halves the pane beside it,
  which ended in a row of slivers: the workspace is laid out again, with the orchestrator
  on the left in the biggest pane and the others in an even grid beside it. `pane_create`
  no longer takes a `direction`; `near` says which pane the new one goes after.
- **Previews and kit tabs are panes of the grid.** What the side preview panel showed, a
  native kit tab, an image, code or text, Markdown, a 3D model, opens as a pane beside the
  terminals, with a terminal pane's header: drag it onto another pane, resize it with the
  dividers, zoom it, close it, reach it with `Ctrl`+`1`…`9`, which count every pane on
  screen. Build your own layout of terminals and kit tabs.
  - A new one opens beside the focused pane, split along its longer side as the + button
    splits; one already open takes the focus where it is. What an agent opens or closes
    lays the workspace out again, as its panes do.
  - A PDF, a web page, a video, a sound or an HTML kit tab still shows in a window of the
    helper: its pane holds a card with the button that brings the window forward, and
    closing the pane closes the window.
  - A workspace holds 16 panes, terminals and previews together. A preset sets the number
    of terminals and keeps every preview.
  - Kit tabs come back after a restart as the panes they were, where you left them. Files
    do not come back, as before.
  - The panel is gone with its tab strip, its drawer, its expanded mode, its toolbar button
    and its shortcut (`Mod+Alt+P`). 1.x still reads the file: it keeps the panes as
    `viewPanes`, a field it does not know, and lays out its terminals without them.
- **A shorter toolbar.** The four layouts are one button with a menu, and the activity
  capsule in the middle is gone: the pane's own header and the inspector say what it is
  doing.
- **No limit on the workers.** The orchestrator decides how many workers run at once and
  how many times a task is tried: **Workers at once** and **Attempts per task** are gone
  from the orchestration settings, and Hangar no longer hands a task back to you after its
  second worker or its third failed delivery. 1.x, reading the same file, goes back to its
  defaults, 3 workers and 2 attempts.
- **Faster orchestration.** The delivery check was the slowest step, and most of it was
  rebuilding the same code.
  - **The check has a build folder of its own.** Every check of a workspace runs in one
    folder with a short path (no more paths too long to build in on Windows), put back on
    each merge's files and cleaned of what the last check left, and builds into a target
    folder only checks use. A fresh checkout for every check made the build start over;
    in the same folder, only what changed is rebuilt.
  - **A shared build cache for the workers, if you want it.** It is off by default:
    workers building different branches in one shared folder can mix their builds, so turn
    it on in the orchestration settings only if they rarely build at once. The settings
    show its size and can empty it.
  - **Workers build what they touch.** They iterate with targeted builds and tests, and
    the full check runs once per delivery.
  - **A check that could not fail is skipped.** A delivery that changes only
    documentation (Markdown, text, `docs/`, licence and changelog files, issue templates)
    runs no check, and neither does one whose merged files are exactly a tree that already
    passed the same check. The review still runs, and the delivery says why.
  - **The orchestrator sees what the check costs.** Its briefing gives the last check's
    time and verdict ("last check 12 min, passed").
  - **Leftovers are removed.** At start, Hangar removes the delivery folders earlier
    versions left in the temporary folder and has git forget the worktrees whose folder is
    gone.
- **The orchestrator has a free hand.** Your instructions come first: its role says so at
  the top, and everything else in it is a default, not a wall.
  - It does small things itself (the changelog, docs, configuration, a fix to the check),
    commits, and can merge a finished branch into the target branch without waiting for the
    delivery queue. It decides when a delivery check and a review are worth their minutes.
    It pushes, tags or releases when you ask, never on its own.
  - A worktree per task is its choice now: `worker_spawn` with `worktree: false` starts a
    worker in the workspace's own folder, beside the others, with no branch and no
    delivery.
- **No worktree left behind.** After a delivery lands, what Windows kept of its folder (a
  `node_modules`, read-only files, paths too long) goes too, and the worktrees nobody uses
  any more, merged, clean and with no pane, are removed with their branch when Hangar
  starts or a workspace opens.
- **Not there yet.**
  - Windows and macOS are built and tested by CI, but nobody has used 2.0 on them yet. Linux
    is where it was used.
  - No screen-reader support (iced has no accessibility tree yet), and buttons do not take
    the keyboard focus: `F6` moves between regions, and the command palette reaches every
    command.
  - Text cannot be selected with the pointer in the diff, in native kit tabs and in most
    panels; a Copy button stands in. Terminals, code and Markdown previews select as before.
  - PDFs, web pages, video, audio and HTML kit tabs open in a window of their own rather
    than in their pane. On GNOME under Wayland, one that an agent opens waits for your click
    on **Open in a window**.
  - On Linux, video and audio previews may not play.
  - Dropping a file on a pane does not work under Wayland.
  - The 3D preview does not read Draco, meshopt or KTX2 compressed glTF files, nor USDZ, and
    plays no FBX animation.
  - A window last left on a second screen reopens centred on the first.

## [1.12.1] - 2026-11-03

- **Terminals no longer lag on Linux.** On Linux the webview is WebKitGTK, where drawing a
  terminal with WebGL was slow and, with some NVIDIA drivers, broken: typing, spaces and
  deletions came late, and a dead key followed by a letter could leave the accent stuck
  (`'` then `e` on a US International keyboard). Terminals are now drawn without WebGL
  there. Windows and macOS are unchanged.
  - To get WebGL back on a Linux machine where it is fast, run
    `localStorage.setItem("hangar.webgl", "1")` in the developer tools; `"0"` turns it off
    anywhere.
- **An Experimental page in the settings.** A place for features that are not finished: off
  until you turn them on, and free to change or disappear. The page stays hidden until a
  version ships one, so nothing shows up yet.

## [1.12.0] - 2026-11-02

- **Tell it where.** Ask any Claude to create a workspace from a repository and say where
  it goes, on any drive: "put it on my second SSD". The clone lands in that folder, and
  missing folders are made. An empty folder already there is used, and put back empty if
  the clone fails. Your **Projects folder** setting is not changed.
  - A repository that is not yours still asks for your click first, and the card now
    shows where it would go. For your own, the agent tells you the folder.
  - Still refused, with a message saying what to do next: a relative path, a path with
    `..`, the operating system's folders and Hangar's own data. A folder that already
    holds something else is refused untouched.
- **Blocked tasks.** A task that waits on something outside the board (a package release,
  a decision, a date) can be blocked with a reason, from the Board or by an agent with
  `board_block_task`. It goes back to To do holding no worker and no files, and agents are
  no longer handed it. The tasks that depend on it keep waiting.
  - Give it a day to look again: the day after, it unblocks itself.
  - Or choose **Only a person can unblock it**: no agent can lift that block, and the
    day only reminds you.
  - A worker that blocks its own task is told to stop.
  - Unblock from the Board, or with `board_unblock_task`. The Board can filter by blocked
    state, and the activity log records each block and unblock.
- **`board_next_task` skips tasks that need you.** Agents are no longer handed them.

## [1.11.1] - 2026-11-02

- Hangar's repository moved to hangar-ai/hangar, with the kits. Old links redirect, and updates keep coming.

## [1.11.0] - 2026-11-02

- **A workspace from a Git repository.** Ask any Claude "create a workspace for
  `owner/repo`", or pick **From a Git repository** when you create a workspace. Hangar
  clones the repository and starts an orchestrator in it. The orchestrator reads the
  project, installs its dependencies, sets the check command, runs the tests once, sums
  up, and waits for your objective. It changes no code.
  - If the repository is not yours or one of your organizations', an agent's request
    first asks you for a click: **Clone** or **Refuse**. Claude Code's own question about
    trusting the folder stays, and you answer it yourself.
  - If it is yours, Hangar records that trust for that folder alone, in Claude Code's
    config.
  - A new **Projects folder** setting chooses where the clones go.
  - Each answer to the click is written in the activity log.
- **Open with VS Code** when you right-click a workspace.
- **Kit tabs use less memory.** Tabs restored at startup load only when you first show
  them.

## [1.10.0] - 2026-11-01

- **The git branch in each pane's header.** You see at a glance which branch a pane's
  folder is on.
- **Worktrees, easier to keep tidy.**
  - Remove the worktrees whose branch is already merged in one go, from the worktree
    panel.
  - **Bring changes back** moves a worktree's uncommitted work into the main checkout.
    If a file was modified on both sides, nothing is moved.
  - A workspace can run a command after a worktree is created and another before it is
    removed, workers' worktrees included. Only you set them. They run on this machine,
    as you, and a command that fails or takes more than five minutes is reported.
- **Fixed: Board, shortcuts and Hangar Mobile.**
  - Moving Board cards by drag and drop now works on Windows.
  - The fullscreen shortcut works again.
  - Sending a file from the phone in Hangar Mobile finds its workspace's folder.
- **Fixed: sessions and panes.**
  - Compressed Codex sessions are now read by the session picker and the context gauge.
  - A pane pinned as orchestrator gets Hangar's tools: it restarts on the same
    conversation.
  - A pane moved to another window while it launches still starts its agent.
  - Launch notes stay visible when Claude Code starts, in a bar under the pane's header
    that you can dismiss.
  - After a delivery on Windows, worktree folders are no longer left on disk.
- **Fixed: the global memory.**
  - It warns you when it nears its limit, and never deletes anything by itself.
  - `memory_list` takes a limit.
  - Creating an entry with a title already taken asks before replacing it.
- **Fixed: agents' tools.** `board_list_tasks` and the other tools that filter ignore an
  empty filter instead of matching nothing.

## [1.9.1] - 2026-11-01

### Fixed

- The review of a delivery now runs on the Claude account of its workspace, as its panes do, not on the machine's default login. When that account hits its usage limit, the delivery says so plainly, and it no longer counts as a failed delivery.

## [1.9.0] - 2026-11-01

- **A kit Library.** **Settings → Kits → Library** lists the kits of the
  [hangar-ai](https://github.com/hangar-ai) organization, read from a static file: no
  server, nothing of yours sent.
  - Each kit is pinned to a tag the catalog names. An update reaches you only when the
    catalog names a new tag.
  - The organization's kits carry a **Verified** badge. Any other kit says that Hangar
    did not review it. Installing and accepting work the same for both.
- **Public kits**, ready to install from the Library.
  - **Guardrails** refuses dangerous commands and the reading of secrets.
  - **Editor**: an agent that needs a secret opens the `.env` for you to fill in, and
    never sees the value.
  - **Checklist**: you work through the manual checks an agent lists, and the result
    goes back to it.
  - **Annotate**: circle a problem on a capture, and the agent gets the image and your
    notes.
  - **CI status** follows a GitHub Actions run after a push.
  - **Tests** shows the last test run per project and agent.
  - **GitHub** shows pull requests and issues, and hands any of them to an agent.
  - **Dependencies** lists vulnerable and outdated packages, and hands them to an agent.
  - **Services** shows listening ports and Docker containers, tied to the worker they
    belong to. It stops nothing: an agent is asked to.
  - **TODO** groups the `TODO`, `FIXME`, `HACK` and `XXX` comments of the project.
  - **Database** is a read-only look at a SQLite file of the workspace.
- **Kits that do more.**
  - An agent, or a hook of a kit, can open one of the kit's tabs.
  - A kit can declare commands its tabs run. You accept them when you enable the kit.
  - A kit can be turned on for **all workspaces** from Settings → Kits, accepted once.
- **You see everything a kit starts.** Before enabling it, you see all that its plugin
  runs by itself: hooks, commands, MCP servers.
  - A kit from GitHub that runs more than hooks is asked again, once, after this update.
    Until you accept it, it stays out of new panes of the workspace.
- **Hangar UI, for kit makers.** A shared look for kits' tabs, following your theme and
  language, described in [docs/kit-ui.md](docs/kit-ui.md). The rest is in
  [docs/kits.md](docs/kits.md).
  - A kit's SVG icon now takes the theme's colour, so it stays visible on a dark theme.

## [1.8.0] - 2026-10-31

- **Kits can do more, and show more before you accept them.**
  - A kit's tabs can keep their own data, in a folder of the kit outside your projects.
    A kit can never reach another kit's folder.
  - A kit's tab can talk to a local server on a port it declares, such as a game editor's
    bridge. You accept each port when you enable the kit. Hangar makes the call: the
    tab's page still has no network of its own.
  - Before you enable a kit, you now see the hooks of its Claude Code plugin, exactly as
    they will run, since Claude Code runs them by itself. A kit from GitHub whose hooks
    change in an update stays out of your agents until you accept it again. A kit linked
    from a folder, which you are writing yourself, is not asked again.
  - The details for kit makers are in [docs/kits.md](docs/kits.md).

## [1.7.0] - 2026-10-31

- **Claude accounts.** Use several Claude subscriptions from one Hangar — say a work Max
  and a personal one — and give each workspace its own.
  - **Settings → Claude accounts** adds an account: name it, and a pane opens for you to
    log in through your browser. Rename, remove, and pick the default account there.
  - Each workspace can have its own account, from its right-click menu in the sidebar
    or when you create it. A workspace that is not on the default shows a small badge
    with the account's name.
  - The toolbar's account menu shows the account of the workspace on screen and each
    account's 5-hour and weekly usage. One click moves this workspace to another
    account, or every workspace that follows the default. Hangar then offers to
    restart the workspace's agents now, with their conversations kept, or at their
    next launch.
  - A single pane can run on another account too, from its menu.
  - Every account has the same setup: conversations, settings, skills, plugins and MCP
    servers come from your main Claude Code configuration. Only the login differs.
    Change the setup from a pane on the main account: the other accounts copy it again
    at each launch.
  - Hangar never switches account by itself, not even when a limit is reached.
  - On macOS, each account keeps its own login in the Keychain.

## [1.6.0] - 2026-10-31

- **Kits.** A kit brings a domain — a game engine, a framework, a studio's own tools —
  into the workspaces you link it to, without Hangar knowing anything about that
  domain. A kit can bring a Claude Code plugin (skills, commands, agents and MCP
  servers), tabs of its own, and setup commands.
  - **Settings → Kits** adds a kit from a folder, linked and read live while you make
    it, or from a GitHub link (a whole repository or one of its folders), checks for
    updates, updates and removes kits.
  - Enable a kit on a workspace from its right-click menu in the sidebar (**Kits…**) or
    when you create it. Before anything is enabled you see what the kit brings, the
    permissions its tabs ask for in plain words, and its setup commands exactly as
    they will run.
  - Setup commands run only when you click **Run**, in a visible pane, once per
    machine. A command that changes in an update is asked again.
  - Every Claude Code pane of the workspace, workers included, gets the kit's skills
    and MCP servers, also when the workspace picks its own MCP servers. Nothing is
    written into your project, and other workspaces are untouched.
  - A kit's tabs open in the side panel, from the toolbar's **Kits** menu or the command
    palette. They run isolated, with no network, and talk to Hangar only through a
    small API, with the permissions you accepted: see the workspace and its panes,
    write to its Claude Code agents, read its text files, open a file in the preview,
    raise a notification.
  - How to make a kit: [docs/kits.md](docs/kits.md).

## [1.5.0] - 2026-10-29

- **See the app an agent is building, on the phone.** In Hangar Mobile, a project whose
  worker runs a dev server shows **See the app**: it opens full screen, live, with hot
  reload, and a button to open it in Safari. Workers' servers are found by themselves
  on their own ports. Under **Settings → Mobile** you can add ports of your own for
  each workspace, such as your own `pnpm dev`.
  - Each app gets an HTTPS port of its own on your Tailscale address (8443 to 8448).
    Only your paired phones get through, and the dev server never sees the phone's
    session.
  - The first time an app is opened, Hangar adds that port to Tailscale Serve, which
    keeps it after a restart.
- **Merge from the phone.** A delivery ready to merge opens on the phone with the files
  it changes, their diff, the check's result and the review's verdict. **Merge** lands
  it, or says what stands in the way. **Send back** gives the worker an instruction to
  fix before it delivers again.

## [1.4.0] - 2026-10-29

- **Stats on the phone.** Hangar Mobile has a **Stats** tab with your agents' usage. Your
  plan's limits come first, when Claude Code reports them: how much of the 5-hour and
  weekly windows is used, and when each resets. Below them are the figures of the
  desktop's statistics page, computed the same way: a period (7, 30 or 90 days, or all),
  tokens or the API-equivalent cost, a chart per day, and rankings by project, agent and
  model.

## [1.3.0] - 2026-10-29

- **Preview any file.** The 3D panel becomes **Preview**: one tab per open file, whatever
  its kind. Agents show you a render, a screenshot, a recording, a report or a page with
  the new `preview_show` and `preview_close` MCP tools.
  - Images zoom and pan, and video and audio play with loop and speed.
  - Code and text get syntax highlighting, Markdown is rendered with a source view, and
    PDFs open in the panel or in your default app.
  - HTML pages run in an isolated sandbox that reaches neither Hangar nor your other files.
    Their scripts and styles must be inline.
  - 3D models work as before.
- **Shares open in place on the phone.** In Hangar Mobile, a file an agent shares opens
  right in **Shared**, whatever its kind: video and audio play, text and code are
  highlighted, Markdown renders, a PDF opens in the phone's viewer, and an HTML page
  renders isolated. Images open in a zoomable view.
- **Wake a sleeping project from the phone.** An asleep project has a **Wake up** button.
  A message, an answer or a permission sent to one of its agents now waits until that
  agent's session has resumed, instead of being lost while it started.
- **The phone layout holds on iPhone.** The app no longer shifts down, under the Dynamic
  Island, and pushes its bottom bar and composer off screen after the keyboard or a
  return to the app.
- Agents' instructions now mention `preview_show` and `phone_share`.

## [1.2.0] - 2026-10-29

- **A real chat on the phone.** In Hangar Mobile, a Claude Code pane (and the project
  manager) now opens as a conversation instead of a copy of its terminal: your messages,
  Claude's answers with their formatting, and between them one line per action ("Edited
  src/a.ts", "Ran pnpm test") that opens its detail. Older messages load as you scroll up.
  Codex, Gemini CLI and shells keep the terminal view, and a **Terminal** button still
  shows any pane's raw screen.
- **Claude's questions as buttons.** When Claude asks you something, the phone shows each
  question with its options as large buttons, several choices when it allows them,
  "Other" with a text field, and Next / Send across several questions. Your answer
  reaches the session on the PC as if you had typed it there.
- **Permissions say what they are for.** A permission shows the tool and what it will do
  (the command, the file) with Allow and Deny.

## [1.1.0] - 2026-10-28

- **Hangar Mobile.** Follow and steer your projects from your phone while Hangar keeps
  running on your PC. It is a web app that Hangar itself serves and that you add to the
  phone's Home Screen: no app store, no account, no server of ours in between. Set it up
  in about a minute under **Settings → Mobile** (see the README).
  - **For you** lists what waits for you in every project: an agent asking for a
    permission, a question for you, a delivery ready to land.
  - Read any pane's screen as it updates, answer an agent, allow or deny a permission,
    press `Esc` or `Enter`, attach a photo or a file, and talk to the project manager.
  - Agents can share images and files with your phone through the new `phone_share`
    MCP tool; images show in **Shares**.
  - Push notifications reach the phone even with the app closed, sent by the PC itself.
  - **Since your last visit** shows what happened while you were away, kept across
    restarts for 30 days. For a few days away, two options: keep the PC from sleeping
    while Hangar Mobile is on, and start Hangar when you log in.
  - Security: it listens on `127.0.0.1` only and is reached through Tailscale Serve over
    HTTPS; each phone pairs with a one-time code and can be revoked. Off by default.

## [1.0.3] - 2026-10-28

- **Project manager.** One agent to talk to instead of every project's agents. Open it
  from the top of the sidebar or the command palette: Hangar creates an HQ workspace
  with its own folder and one Claude Code agent (Opus by default) that follows every
  workspace, with or without an orchestrator.
  - Ask it where your projects stand: agents, boards, what waits for you, deliveries
    ready to land, each plan's status.
  - Ask it to pass an instruction on: it is queued for that project's orchestrator,
    and a sleeping workspace wakes up.
  - It speaks up by itself only for what waits for you: an agent waiting for an answer
    or a permission, a task marked needs-you, a delivery ready to land, a board whose
    tasks are all done.
  - It keeps a tracker of what you want from each project, handed back after every
    compaction. It never writes code, never acts on its own, and the HQ never sleeps.
- **New MCP tools for it:** `project_list`, `project_status` and `project_send`, which
  only the project manager can call.

## [1.0.2] - 2026-10-28

- **Orchestrator mode without git.** A workspace that is not a git repository, such as
  a UEFN island, can now run workers. They start in the workspace's own folder, side by
  side, with no worktree and no branch. When a worker delivers, nothing is merged,
  checked or reviewed: its task is done and its pane closes. Since nothing keeps two
  workers' changes apart there, a task that does not list the files it owns never runs
  beside another one. The orchestrator no longer asks you to make the folder a
  repository.
- **A pane that just started is no longer shown as working.** A Claude Code pane stayed
  "working" until its first turn, in the navigator, in `pane_list` and for sleep, so a
  workspace of fresh sessions never went to sleep. It now shows idle once its session
  is ready.

## [1.0.1] - 2026-10-27

- **Put a workspace to sleep by hand.** Right-click a workspace in the navigator, or run
  **Put to sleep** from the command palette. It is the same sleep Hangar applies on its
  own, without the idle delay, and with the same guards: the entry is greyed out, with
  the reason, while an agent works or waits for you, a shell runs a process, an agent
  could not resume its session, or a worker or a delivery is running. The workspace on
  screen hands over to the one you used last.
- **A delivery waiting for you can be redone.** When a worker fixes its branch after its
  delivery passed and waits to be merged, `delivery_submit` withdraws the waiting one and
  runs the whole chain again. **Merge** never lands a version the worker changed since it
  was tested, and the reason a merge was refused stays on screen.
- **No personal path on the launch line.** Orchestrator and worker panes read their MCP
  configuration from `HANGAR_MCP_CONFIG` instead of typing its full path, so a screen
  share or a recording no longer shows your user folder.

## [1.0.0] - 2026-10-27

The first stable release. It adds orchestrator mode on top of what the 0.12 to 0.15
releases brought the day before: the Pro interface, the 3D preview, usage
statistics, workspace sleep and per-workspace MCP servers.

- **Orchestrator mode.** Pin one Claude Code pane per workspace as its orchestrator
  (Opus by default). It plans on the board, with owned files, labels and dependencies
  per task, and delegates the work instead of writing code.
- **Workers.** `worker_spawn` claims a task, checks out a git worktree on a branch named
  after it, and opens a Claude Code worker there (Sonnet by default) with a brief, its
  own port range (`HANGAR_PORT_BASE`, `HANGAR_PORT_COUNT`) and its model.
  `worker_list` and `worker_stop` complete the set. Two tasks that own the same files
  cannot be in progress at once.
- **Delivery queue.** `delivery_submit` merges a worker's branch onto the target's
  current head in a temporary worktree, runs the workspace's check command if one is
  set, has a fresh `claude -p` session with read-only tools review it, then lands it
  locally. Hangar never pushes. A failure goes back to the worker with the conflicting files
  or the failing line; repeated failures are handed to you.
- **Event-driven orchestrator.** Board moves, comments, workers that wait for you, stop
  or exit, and delivery outcomes reach the orchestrator as batched messages, so it
  never polls.
  Orchestrators and workers get their role and the workspace's state when a session
  starts, resumes or is compacted.
- **"Needs you" inbox.** Agents waiting for an answer or a permission, tasks handed back
  with a question, and deliveries waiting to land, across every workspace.
- Per-workspace orchestration settings: maximum workers, attempts, automatic landing (on
  by default),
  check command, models, target branch and permission mode.
- A prompt queued by an agent or by Hangar is labelled with its sender, and the
  inspector shows who queued it.
- A workspace stays awake while its workers run or one of its deliveries lands.
- PowerShell panes start without the PowerShell banner.
- The workspace board (`.hangar/`) is kept out of `git status` through the repository's
  local `.git/info/exclude`.

## [0.15] - 2026-10-26

- Each workspace has its own 3D preview; a model shown in a workspace you are not
  looking at waits there, and the agent is told whether you can see it.
- Sidebar folders read as headings, and each workspace row sums up its agents.
- Changes agents make to the layout show as news rather than as a warning.
- The workspace path left the toolbar.
- A Dockerfile to self-host the website.

## [0.14] - 2026-10-26

- **Usage statistics**: a view built from Claude Code and Codex transcripts on disk,
  with tokens, API-equivalent cost, rankings by project, agent and model, and an
  activity heatmap. Prices are API list prices, not a plan's price.
- **Workspace sleep**: an idle workspace has its processes ended after a delay you set,
  and its agents resume their sessions when it is opened again. Agents acting on a
  sleeping workspace over MCP wake it.
- **MCP servers per workspace**: a panel lists the servers Claude Code would load in the
  workspace and lets you pick which ones its Claude Code panes start with.
- hangar-bridge no longer holds an agent on a file another pane just edited: the edit
  goes through and the model is told who edited it before.
- Closing a pane can end every process its shell started.

## [0.13] - 2026-10-26

- **3D preview over MCP**: `model_show`, `model_snapshot` and `model_close` let an agent
  open a model in a panel beside the terminals and render it to a PNG to check its work.
- Reads glTF/GLB, OBJ with MTL, STL, PLY, FBX and USDZ, only from the workspace's
  folders and its git worktrees, and reloads when the file changes.
- Captures of the preview can be saved where you choose.

## [0.12] - 2026-10-26

- **The Pro interface**: a three-column window with a navigator, the work area and an
  inspector, on a new default theme, Graphite (33 themes in all).
- The navigator shows workspaces as a tree with their agents, plan meters and a filter
  for the ones waiting on you.
- The inspector shows the focused pane's session, context and cost, board task, prompt
  queue, changed files and recent activity.
- A command palette (`Mod+Shift+P`) finds panes, commands and workspaces, in English or
  French.
- Keyboard access throughout: F6 walks the regions, pane boundaries resize from the
  keyboard, and focus returns to where a dialog was opened from.
- Contrast held to 4.5:1 for secondary text, light themes included; one icon set
  across the window.
- Side panels without room become drawers; a layout preset asks before closing busy
  panes.

## [0.11] - 2026-10-07

0.11.0 (2026-10-06), 0.11.1 (2026-10-07).

- Every pane has a first name unique across all workspaces, and Claude Code sessions
  run as "Ava (Shop)" so neighbouring agents know which project they work on.
- A pane resumes only a conversation that was actually written, and starts fresh
  otherwise, instead of failing with "No conversation found".
- A Tidy button lays the open panes out as an even grid without restarting anything.

## [0.10] - 2026-10-05

- **Pane tools over MCP**: `workspace_list`, `pane_list`, `pane_create`, `pane_close`,
  `pane_restart`, `pane_send`, `pane_read`, `pane_focus`, which a setting can turn off.
- A workspace can move to a window of its own, on another screen, with its agents
  still running; `workspace_window` does the same over MCP.
- Agents sign the board with their pane's name.
- `HANGAR_ENDPOINT_FILE` lets a development build run beside an installed Hangar.

## [0.9] - 2026-10-03

- **hangar-bridge**, a Claude Code mod loaded into Hangar's panes: it reports turns,
  permission dialogs, context, plan usage and edits, and submits prompts held until
  the agent is free.
- Agent state (working, waiting for you, done) in each pane header and per workspace
  in the sidebar; plan usage at the bottom of the sidebar.
- Reset every agent of a workspace to a new conversation.
- Zoom a pane, search a terminal, drop a file or image onto a pane to paste its path.
- WebGL rendering in the terminals.
- Board cards with assignee, blockers and filters; the board as a side panel; a diff
  view of what changed in a pane's folder or worktree.
- The Groq key moved to the system keychain.

## [0.8] - 2026-10-03

- A gauge of how full each agent's context window is.
- Saved commands, run from any pane.
- Folders to organise workspaces in the sidebar.

## [0.7] - 2026-08-11

- **Git worktrees**: create, open and remove linked worktrees from a panel.
- **Global memory over MCP**: `memory_*` tools give every agent a store of facts shared
  across workspaces, with a view to browse it.
- Clicking a desktop notification brings Hangar back on the right pane (Windows).
- The website and its browser demo joined the repository.

## [0.6] - 2026-08-10

0.6.0 and 0.6.1, both 2026-08-10.

- Dictation transcribes in seconds rather than minutes, on a smaller local Whisper
  checkpoint.
- whisper.cpp is built for AVX2 instead of the build machine's CPU, so releases run on
  CPUs without AVX-512.
- The custom Windows installer's buttons work.
- MCP registrations pointing at a missing executable are reported as stale and can be
  repaired; internal identifiers moved from `iabench` to `hangar` with fallbacks.

## [0.5] - 2026-08-09

0.5.0 (2026-08-08), 0.5.1 and 0.5.2 (2026-08-09).

- **Dictation**: hold a key, speak, and the text is typed into the focused pane. It
  runs locally on whisper.cpp, with Groq's hosted Whisper as an optional fallback.
- Dictation has its own section in the shortcut settings.
- One Windows installer, the per-user `.exe`, which needs no administrator rights.

## [0.4] - 2026-08-07

0.4.0 and 0.4.1, both 2026-08-07.

- macOS (universal `.dmg`) and Linux (`.AppImage`, `.deb`, `.rpm`) builds.
- Open a VS Code `.code-workspace` file; its extra folders are passed to Claude Code.
- A launch command of your own per agent, for wrappers and aliases.
- A new pane starts with the agent of the pane it was split from.
- Closing a pane no longer freezes the window.
- Shift+Enter inserts a newline in agent panes and still submits in a plain shell.

## [0.3] - 2026-08-05

- Renamed **Hangar.AI**, after shipping as the misspelt Hangar.IA in 0.2.0; updating
  replaces the old install and keeps its data.
- A logo, for the app icon, the installer and the sidebar.
- Twenty more themes ported from popular VS Code palettes, 32 in total.
- Settings rebuilt around a searchable list, with rebindable shortcuts.
- Discord Rich Presence, off by default.

## [0.2] - 2026-08-05

- Renamed from IaBench to Hangar.IA.
- The pane grid rewritten so a layout change no longer restarts every terminal: split,
  close, drag to move or resize, up to 16 panes, each with a first name.
- A desktop notification when a pane falls quiet after real work.
- Switching workspace no longer kills their shells.
- A custom Windows installer.

## [0.1] - 2026-08-05

0.1.0 and 0.1.1, released as IaBench, Windows only.

- Workspaces with saved pane layouts and a per-pane agent or shell.
- Real terminals in a resizable split, with 12 themes.
- Session resume for Claude Code and Codex.
- A shared task board per workspace, served to agents over MCP by the same binary run
  with `--mcp`, with atomic claims.
- One-click MCP registration for Claude Code, Codex, Gemini CLI, Cursor and VS Code.
- Automatic updates from signed GitHub releases.
- An interface in English and French (0.1.1).

[2.5.1]: https://github.com/simnJS/hangar-ai/releases/tag/v2.5.1
[2.5.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.5.0
[2.4.6]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.6
[2.4.5]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.5
[2.4.4]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.4
[2.4.3]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.3
[2.4.2]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.2
[2.4.1]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.1
[2.4.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.4.0
[2.3.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.3.0
[2.2.1]: https://github.com/simnJS/hangar-ai/releases/tag/v2.2.1
[2.2.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.2.0
[2.1.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.1.0
[2.0.0]: https://github.com/simnJS/hangar-ai/releases/tag/v2.0.0
[1.0.1]: https://github.com/simnJS/hangar-ai/releases/tag/v1.0.1
[1.0.0]: https://github.com/simnJS/hangar-ai/releases/tag/v1.0.0
[0.15]: https://github.com/simnJS/hangar-ai/releases/tag/v0.15.0
[0.14]: https://github.com/simnJS/hangar-ai/releases/tag/v0.14.0
[0.13]: https://github.com/simnJS/hangar-ai/releases/tag/v0.13.0
[0.12]: https://github.com/simnJS/hangar-ai/releases/tag/v0.12.0
[0.11]: https://github.com/simnJS/hangar-ai/releases/tag/v0.11.1
[0.10]: https://github.com/simnJS/hangar-ai/releases/tag/v0.10.0
[0.9]: https://github.com/simnJS/hangar-ai/releases/tag/v0.9.0
[0.8]: https://github.com/simnJS/hangar-ai/releases/tag/v0.8.0
[0.7]: https://github.com/simnJS/hangar-ai/releases/tag/v0.7.0
[0.6]: https://github.com/simnJS/hangar-ai/releases/tag/v0.6.1
[0.5]: https://github.com/simnJS/hangar-ai/releases/tag/v0.5.2
[0.4]: https://github.com/simnJS/hangar-ai/releases/tag/v0.4.1
[0.3]: https://github.com/simnJS/hangar-ai/releases/tag/v0.3.0
[0.2]: https://github.com/simnJS/hangar-ai/releases/tag/v0.2.0
[0.1]: https://github.com/simnJS/hangar-ai/releases/tag/v0.1.1
