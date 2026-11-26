# Install and getting started

[← Hangar](../../README.md) · [Guide](README.md)

Download the file for your system from the
**[latest release](https://github.com/hangar-ai/hangar/releases/latest)**, under
**Assets**. You only download once: after that, the app offers each new version and
installs it in one click. An install of 2.x or 1.x offers 3.0 the same way.

| System | File | Notes |
|---|---|---|
| Windows (x64) | `Hangar.AI_<version>_x64-setup.exe` | Installs for your user only, without administrator rights. Its release notes say whether it is code-signed, and by whom. HTML kit tabs and PDFs use WebView2, which ships with Windows 11; on Windows 10 the installer adds it if it is missing. |
| macOS (Apple Silicon and Intel) | `Hangar.AI_<version>_universal.dmg` | One universal build for both, signed and notarised, macOS 10.15 or later. |
| Linux (x86_64) | `.AppImage`, `.deb` or `.rpm` | The AppImage runs without installing (`chmod +x` it first) on distributions with glibc 2.35 or later (2022 on). HTML kit tabs and PDFs need WebKitGTK 4.1: apt and dnf install it with the `.deb` and `.rpm`; with the AppImage, install `libwebkit2gtk-4.1-0` (Debian, Ubuntu) or `webkit2gtk4.1` (Fedora) if your system lacks it. |

> [!NOTE]
> **Windows: SmartScreen.** A signed release (its notes say "Signed by …" on the Windows
> line) shows that publisher's name when it starts, and the app it installs is signed too.
> SmartScreen also goes by reputation: for the first downloads of a publisher it does not
> know yet, it can still show "Windows protected your PC". Click **More info**, check that
> the publisher is the one the notes name, then **Run anyway**. The warning fades as
> downloads build up. A release whose notes say "Unsigned" shows "Unknown publisher"
> instead: the same **More info**, then **Run anyway**.
>
> **macOS: Gatekeeper.** The macOS build is signed and notarised, so it opens after
> macOS's usual first-launch confirmation. If a release's notes ever say the macOS build is
> unsigned, macOS blocks the first launch: right-click the app and choose **Open**. On
> macOS 15 and later, go to **System Settings → Privacy & Security** and click **Open
> Anyway**.

HangarAI runs coding agents in its panes: [Claude Code](https://code.claude.com/docs/en/overview),
[Codex](https://learn.chatgpt.com/docs/codex/cli) (beta), or both; either one is enough. If
neither is installed, HangarAI offers to install them the first time it opens (see below).

## The trial, then a plan

HangarAI starts with a **14-day free trial**. Its first screen asks you to sign in with a
HangarAI account (**Continue with GitHub**, in your browser); no card is asked, and an account
has one trial. The trial includes 150 HangarAI credits for the hosted features.

Then it takes a plan, VAT included, paid through Stripe
([pricing](https://hangarai.dev/pricing/)):

| | HangarAI Pro | HangarAI Max |
|---|---|---|
| Price | 20 € a month, or 192 € a year | 39 € a month, or 374 € a year |
| HangarAI credits | 3,000 a month | 10,000 a month |
| [Cloud Builds](cloud.md) and [hosted nodes](hosted-nodes.md) | up to 3 machines at once, together | up to 5 machines at once, together; Cloud Builds first in the queue |
| Computers | 3 | 3 |

Both have the whole app, every update and every hosted feature: dictation, image generation,
share links, Cloud Builds, hosted nodes and Hangar Mobile from anywhere ([HangarAI Cloud](features.md#hangarai-cloud)).
There is no Lifetime license.

- **Your key.** Enter the key you receive in **Settings → License**, or sign in there to find
  a license bought with your HangarAI account. The page shows your plan, its renewal, its
  computers and what each hosted feature costs; **Upgrade to Max** and **Manage
  subscription** (change or cancel the plan, the card, the invoices) open your subscription at
  Stripe, and a change between Pro and Max is prorated.
- **Credits.** The monthly credits do not roll over. Packs of 1,000 (3 €) and 5,000 (10 €)
  never expire, and **pay as you go**, off until you turn it on in Settings → License, keeps
  hosted features running past your credits up to a monthly limit you set. Every cost shows
  in credits with its euros beside it before you spend.
- **Computers.** A license runs on 3 computers. **Manage computers** (Settings → License, or
  hangarai.dev/account) frees a seat for another one.
- **Invite & earn.** Your link, from hangarai.dev/account, earns you 30 % of what the people
  you bring pay, for as long as they pay; they get 1,000 credits.
- **Without a license**, once the trial is over, HangarAI shows its license screen only: your
  workspaces, board and memory wait for you, untouched.
- **Installs of 2.5.1 and earlier** keep HangarAI for free: at their first launch of 3.0 you
  sign in with GitHub (no card, nothing to pay) and get a free license on your HangarAI
  account for that computer, with 30 credits a month for dictation.

HangarAI checks the license online when it starts and about once a day (the license, a hash
of the machine and the app version, as the [privacy page](privacy.md) says).

### A computer without a screen

On a server, a build machine or any computer you reach over SSH, activate your key from a
terminal. `hangar-ai` is the app's own executable (its full path where it is not on your
`PATH`):

```
hangar-ai license activate
hangar-ai license status
hangar-ai license deactivate
```

- **activate** asks for your license key, `HANGAR-…`, which the page after your purchase
  shows; [hangarai.dev/account](https://hangarai.dev/account) makes a new one when you need it. It reads the key from the
  standard input, so the key stays out of your shell's history: type it at the prompt, or
  pipe it in (`hangar-ai license activate < key.txt`). `hangar-ai license activate <key>`
  works too. The license is kept where the app keeps it, `license.bin` in its data folder
  (under `HANGAR_DATA_ROOT` when you set it), and [`hangar-ai cloud run`](cloud.md) uses it.
- **status** says your plan, until when, and how this computer shows in your account's
  computers. When the license is due for its daily check, it asks HangarAI's server first, as
  the app does when it starts. On a computer where the app never opens, run it now and then
  (once a day from cron is plenty): a license not checked for 14 days stops until it is.
- **deactivate** frees this computer's seat for another one.

The free trial starts in the app, with your HangarAI account: there is no sign-in from a
terminal. The command speaks English, or French when your system's language is French. Its
exit code is 0 when it worked, 75 when HangarAI's server cannot be reached, 77 for a refusal
(a key that is not one, no seat left), and, for **status**, 77 when this computer may not use
HangarAI.

## Coming from an earlier version

Accept the update, or install 3.0 over 2.x or 1.x: it uses the same folders and the same
`state.json`, so your workspaces, boards, memory, settings, shortcuts, kits and accounts are
there. From 1.x, the kit tabs you had left open and each account's cached usage lived in the
old web view, and are not carried over; the [changelog](../../CHANGELOG.md) lists what 2.0
left out, mainly screen reader support, which HangarAI still lacks.

## Getting started

0. **Your agents.** When Hangar finds neither Claude Code nor Codex on this machine, it
   opens a **Set up your coding agents** sheet at start, with a card per agent (Codex
   marked beta); it stands in the way only until one of them is installed.
   **Install Claude Code** runs Anthropic's official installer, exactly as
   [Claude Code's docs](https://code.claude.com/docs/en/setup) give it —
   `curl -fsSL https://claude.ai/install.sh | bash` on macOS and Linux,
   `irm https://claude.ai/install.ps1 | iex` in PowerShell on Windows. **Install Codex**
   runs OpenAI's — `curl -fsSL https://chatgpt.com/codex/install.sh | sh`, or
   `irm https://chatgpt.com/codex/install.ps1 | iex` ([Codex (beta)](codex.md)). Either
   installs for your user only, with no administrator password, and the sheet shows what
   it prints as it runs. If it fails, the sheet says why, shows the installer's output, and
   links to the install guide. Once an agent is found, **Sign in** opens it in a pane,
   where it asks for your account the first time (Claude, or ChatGPT or an OpenAI API key
   for Codex); the sheet notices when you are signed in. **Use Hangar with shells only**
   sets the agents aside: Hangar stops asking, and new workspaces start with a shell. Turn
   it back off in **Settings → General → Shells only, without an agent**.
1. **Create a workspace** with `Ctrl`/`Cmd`+`Shift`+`N` and pick a project folder. The
   workspace starts with one agent pane, its orchestrator: tell it what you want. *Starts
   with* offers Claude Code or Codex, Claude Code by default when both are installed (or the
   one in **Settings → General → Default agents**). Outside a git repository its workers
   share the folder (`git init` gives each one a worktree). **More options** has the other
   setups: the workers' agent, several panes, a layout, shells and agents of your choice.
   With no agent installed, the dialog says so and the workspace starts with a shell.
2. **Connect your agents to Hangar.** Codex panes get Hangar's MCP server at every launch,
   with nothing to set up. For Claude Code, open the MCP panel (`Ctrl`/`Cmd`+`Alt`+`M`;
   `Alt` is `Option` on a Mac) and click **Configure**. The panel can also write a short
   playbook into the project's `CLAUDE.md`, telling agents to claim tasks before they
   start. Restart the agents so they load the server.
3. **Give them work.** Add tasks on the board (`Ctrl`/`Cmd`+`Alt`+`B`), or ask an agent to
   split a goal into tasks. Other agents pick tasks up with `board_next_task`.
4. **Orchestration.** A new workspace has its orchestrator already; in another one, run
   **New orchestrator** from the command palette (`Ctrl`/`Cmd`+`Shift`+`P`), or pin an
   existing Claude Code or Codex pane from its **…** menu. Then describe the goal to it.
   **Orchestration…** in the same menu opens the workspace's settings: models,
   permissions, check command, automatic merging, the delivery speed, where checks run and
   the night shift ([Orchestration](orchestration.md)).

The command palette lists every command. Shortcuts can be changed in
**Settings → Shortcuts**, and the defaults use `Ctrl` on Windows and Linux and `Cmd` on
macOS.

## Troubleshooting

- **Opening HangarAI again shows the window that is already open.** HangarAI runs once per
  data folder: a second launch (the dock, the launcher, a terminal) hands itself over to the
  running one, which brings its window forward, or opens it again if it was closed to the
  background, instead of starting a second copy that would restart your workspaces' agents
  beside the first. If the running one does not answer within 10 seconds, the new launch gives
  up and says so in its terminal: quit HangarAI from its tray icon (or end its process), then
  open it again. A copy started with its own `HANGAR_DATA_ROOT` runs beside it.
