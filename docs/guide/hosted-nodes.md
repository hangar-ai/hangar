# Hosted nodes

[← Hangar](../../README.md) · [Guide](README.md)

A hosted node keeps your workspaces running while your computer is off. **Keep running in
the cloud…**, in a workspace's menu, rents a server for you alone at Hetzner, in Germany or
Finland, puts a `hangar-node` on it, and moves the workspaces you choose there: their
terminals, agents, board and memory keep working, workers keep delivering, and your phone
follows them as it follows any [node](features.md#nodes). Close the laptop. When the session
ends, the work comes back to this computer, and the server is deleted.

It comes with HangarAI 3.0, for **Pro and Max** licenses, paid with your credits by the
started hour. The trial and the free license of installs of 2.5.1 and earlier cannot rent
servers. A hosted node is a node like the ones you add yourself: the same row in Settings ›
Nodes, under a cloud icon and the name **HangarAI Cloud · Falkenstein**, the same marks in
the navigator, the same way to move a workspace.

## Start one

In a workspace's menu in the navigator, choose **Keep running in the cloud…**; or **Start a
hosted node** in the Cloud space (the cloud chip in the status bar). The sheet shows, before
anything is spent:

- **Size**, with its rate:

  | Size | What it has | Rate | For |
  |---|---|---|---|
  | Small | 2 vCPU, 4 GB, 80 GB of disk | 34.0 credits a started hour | one or two workers |
  | Standard | 4 vCPU, 8 GB, 160 GB | 61.4 | most workspaces |
  | Large | 8 vCPU, 16 GB, 320 GB | 119.4 | many workers and heavy builds |

  The vCPUs are shared ones. A heavy build of a hosted workspace can still go to
  [Cloud Builds](cloud.md), as it does from this computer.
- **Until**: when the session ends ("Tomorrow 08:00"), at least 30 minutes and at most 24
  hours from now. It can be extended later.
- **Where**: Falkenstein or Nuremberg, in Germany, or Helsinki, in Finland: Hetzner
  datacenters in the European Union. Falkenstein is the first choice. When no server of the
  size is free there, the sheet says so and lets you pick another place or size. When every
  server of HangarAI Cloud is taken, it says so too, and when one frees up.
- **Agents to install**: Claude Code and Codex, each from its official installer, unmodified.
  You sign in on the server yourself; see [Sign the agents in](#sign-the-agents-in).
- **Workspaces to move there**: the one you opened the sheet from is ticked, and the other
  workspaces of the same repository are offered. Their folders here stay as they are. With
  none of this computer's workspaces to move, the machine starts empty, ready for one.
- **The estimate**: "11 hours · 675.4 credits, reserved now; what you do not use comes back",
  your balance, and, when it does not cover the session, the longest one that does.

The first time, it asks you to accept the
[acceptable use policy](https://hangarai.dev/terms/#cloud-builds) of HangarAI Cloud, which
Cloud Builds has too. **Start** then shows a row in the Cloud space and a machine in the
navigator, lighting up **Creating the server**, **Installing**, **Connecting**: about one to
two minutes. The workspaces you chose are then carried to the node and handed over to it,
with the move any node offers; their rows in the navigator wear the machine's mark.

## What it costs

A hosted node is billed by the **started hour**, as Hetzner bills HangarAI: 10 hours and 40
minutes are 11 hours. The shortest session is 30 minutes, which is billed one hour.

- **A session is reserved whole at its start**: its hours at its size's rate, 675.4 credits
  for 11 hours on a Standard, with the euros beside them. What you do not use comes back when
  the session ends, so **End now** after 5 hours and 10 minutes costs 6 hours, not 11.
- **Rates are fixed when the session starts.** Pro's 3,000 credits a month are about 49 hours
  of a Standard, 88 of a Small.
- **Nothing is charged** when the session never had a server: no server free, the provider
  refusing it, or the server not set up. A server that is lost is replaced once, and the
  hour it lost is refunded. Not enough credits stops the session at the sheet, with the
  longest session that fits.
- **No hours a day or a month.** Your credits are the only limit.

## Several at once, as long as the credits last

A license runs as many hosted nodes at once as its plan runs machines: **3 with Pro, 5 with
Max**. They are the same machines as [Cloud Builds](cloud.md#limits)': a build machine
counts, and a build waits for a lane of one of its machines when the plan's are all taken.
The sheet shows the count before **Start**; past it, it says what runs ("3 machines already
run: 2 builds and 1 node") and **Start** is greyed until you end one.

A session is **extended** for as long as your credits pay. **Extend**, on its row, offers
hours at their price, each time up to 24 hours ahead from now: the session has no total
length. The last 30 minutes show the row as **Ending soon**, with **Extend** first, and a
notification here and on Hangar Mobile says "Your cloud machine stops at 08:00 · Extend 2 h
(122.8 credits)". An extension reserves its own hours.

## What happens at the end

At the hour you chose, or at **End now** (which asks first), the work comes back to this
computer, one of two ways:

- **This computer reaches the node.** HangarAI takes the workspaces back straight over SSH,
  in the session's last two minutes or at **End now**, and nothing is parked. The server is
  deleted.
- **This computer is off, or out of reach.** The node **parks** the work: for each workspace,
  its branches and tags this computer does not have, the work its checkouts had not
  committed, its board, memory and agents' conversations, packed in one parcel and sealed to
  a key drawn on this computer when the session started. Only this computer holds the private
  half, in its keychain (without one, in a file of the app's data folder that only your user
  can read): HangarAI's servers, Cloudflare and the node itself cannot open it again. The parcel goes to Cloudflare R2, in the European Union, and the server is deleted.
  The row says **Waiting for your computer · expires in 7 days**.

  When this computer opens again, the work is **brought back by itself**: "Bringing back shop:
  3 branches, 12 board changes", and the workspaces are this computer's again, as they were
  left. **Restore**, on the row, does it by hand. A parcel is kept **7 days**; then it is
  deleted, and the row says it expired. Once it is back, it is deleted at once.

Work you had not committed when the workspace left goes with it, and the node's comes back
in its place, uncommitted, in the same folders: as long as you did not change a folder here
meanwhile, what it held went to the node, which carried it on.

Either way, what changed on both sides is kept side by side and said: a branch that moved
here and on the node comes back as `<branch>-from-cloud`; uncommitted work that cannot be
put back where it was (you changed that folder here since) waits on a branch
`hangar/parked/<folder>`. Nothing you did on this computer is overwritten.

## Sign the agents in

A hosted node starts with Claude Code and Codex installed, if you ticked them, and no account.
The first worker pane on it opens the agent's own sign-in by itself, with a line above the
pane saying that the sign-in stays on that server and goes with it:

- **In your browser, on the provider's page.** Claude Code runs `claude auth login`, which
  prints an address to open. Codex runs `codex login --device-auth`, which gives a code to
  type on OpenAI's page (to enable in your ChatGPT security settings), since the callback of
  its browser sign-in would reach the server, not this computer. You sign in with your own
  account, as on your PC; the sign-in is written on the node by the agent, and ends with it.
- **An API key of your own**, **Use an API key**, on the node's card: you type it here, it
  goes through the SSH tunnel to the node alone, and it is kept there where the agent reads
  it: by `codex login --with-api-key` for Codex, in a file only the node's user reads, for
  Claude Code. The usage is billed to your API account. **Forget the key** deletes it.

The same card, in **Settings › Nodes** and in the navigator, installs and signs in Claude
Code and Codex on **any** node, a machine of your own reached over SSH as well as a hosted
one: **Install Claude Code**, **Sign in**, **Use an API key**, then **Ready**. Your
subscription's limits are the ones you have on your PC: many workers through a night spend
your own plan as they would here.

## What HangarAI never sees

- **Your code, your files and your conversations.** They go from this computer to the server
  over SSH, as for any node, and come back sealed to this computer's key. HangarAI's server
  holds the session's record only: your license, the size, the place, the agents you chose,
  its times and credits, the public halves of the session's keys, the parcel's size and
  checksum, and a hash of the token the node uses to ask for its upload. Never a file, a
  branch, a board card or a line of a conversation.
- **Your agents' sign-in.** HangarAI never reads `~/.claude/.credentials.json` or
  `~/.codex/auth.json`, never copies those of your PC and sends nothing of them to its server.
  An API key you type goes to the node and nowhere else; it is never printed or logged.
- **The key of your parcel.** It never leaves this computer's keychain.

What a hosted node is, for the rest: a server for you alone at Hetzner, a user that is not
root, only the session's own SSH key let in, and a firewall that opens SSH alone and closes
outgoing mail and mining pools' ports. Hetzner, whose datacenter it is, can in principle
reach the hardware while it runs. A hosted node is not for mining: a server whose processor
Hetzner measures above 85 % for 30 minutes is flagged for a review, and for 2 hours its
session ends, its work is parked, and the license's machines wait for that review, as they do
for Cloud Builds.

The details are on the [privacy page](https://hangarai.dev/privacy.html#hosted-nodes) and in
the [HangarAI Cloud terms](https://hangarai.dev/terms/#cloud-builds). Running on your own
computer, or on your own nodes, stays the default.
