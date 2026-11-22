# Cloud Builds

[← Hangar](../../README.md) · [Guide](README.md)

Cloud Builds runs a command of yours, a test suite, a build, a delivery's check, a
training run, a render, on a machine rented for you alone, as if it ran in your terminal:
your project's files go up, the output comes back as it is printed, and the exit code is
the command's. The machine is the one the work needs: 32 or 64 CPU threads for builds and
tests, a GPU (an RTX 4090, an A100 or an H100, or 2 to 8 of them) for machine learning
and rendering, or exactly the GPU, memory, threads and disk you ask for. It is billed by
the second, in credits, kept a couple of minutes for your next run, then wiped and
destroyed.

It comes with HangarAI 3.0, for **Pro and Max** licenses. The trial and the
free license of installs of 2.5.1 and earlier cannot rent machines.

## Run something

**From the app.** In a workspace, open the command palette (`Ctrl`/`Cmd`+`Shift`+`P`)
and choose **Cloud: Run in the cloud…**. The price sheet shows, before anything is spent:

- the command, the workspace's check command filled in;
- the machine: one of the [classes](#choose-a-machine) or an advanced spec, in a
  certified datacenter in Europe;
- the rate of the best machine free now, and your limit on it; when Europe has none of
  that class free, the rate of the best one elsewhere and **Any region for this run**,
  which says that your files then leave Europe (**Europe only** takes it back);
- your place in the queue, when there is one;
- the time limit: 15 minutes, 30 (the default), 1, 2, 4, 8 or 24 hours; 2 hours by default
  on a machine with a GPU, whose training runs and renders last longer, until you choose
  one;
- how long the machine is **kept afterwards** for your next run: none, 2 (the default), 5,
  10 or 30 minutes. What you choose is the project's setting from then on: the next sheet
  starts with it, and so do the project's runs started from a terminal without `--keep`;
- how many files will be sent, the first of those left out and why, the environment
  variables the project sends (by name, from `.hangar/cloud.env`), and **Forget it** for
  the project's dependency cache;
- an estimate from this project's last runs, your balance, and what it will be after.

The first time, it asks you to accept the
[acceptable use policy](https://hangarai.dev/terms/#cloud-builds). **Run** opens a
terminal pane for the job; **Run on this computer** runs the same command here instead.

**From a terminal**, yours or an agent's:

```
hangar-ai cloud run -- cargo test --workspace
hangar-ai cloud run --time-limit 1h --class 64 -- pnpm test
hangar-ai cloud logs --follow
hangar-ai cloud stop
hangar-ai cloud forget-cache
hangar-ai cloud classes
hangar-ai cloud run --class gpu_rtx_4090 -- python train.py
hangar-ai cloud run --gpu h100 --gpus 2 --vram 80 -- torchrun --nproc_per_node 2 train.py
```

`hangar-ai` is the app's own executable; where it is not on your `PATH`, use its full
path, as the app does when it types the command into a pane. The command runs from the
folder you are in, inside the project's git repository. Its options:

| Option | What it does |
|---|---|
| `--time-limit <time>` | Stops the command past it: 30m by default, 2h on a machine with a GPU; from 1m to 24h. |
| `--keep <time>` | Keeps the machine for your next run: the project's setting by default (2m unless the sheet chose another), up to 30m; `0` destroys it at once. Never longer than HangarAI's server allows, which the run says. |
| `--max-rate <credits>` | Rents only a machine at most this many credits a minute. |
| `--class <id>` | A class of the catalog (`hangar-ai cloud classes` lists them): `threads_32` (the default), `threads_64`, `gpu_rtx_4090`…; `32` and `64` still work. |
| `--gpu <model\|any>`, `--gpus <n>`, `--vram <GB>`, `--cpus <n>`, `--ram <GB>`, `--disk <GB>` | Or the machine said part by part, each the least it needs: a GPU of this model (`rtx_4090`, `h100`; several with commas) or `any`, how many (1 to 8), the memory of each, the threads, the memory, the disk. Not with `--class`. See [Choose a machine](#choose-a-machine). |
| `--any-region` | A machine anywhere a certified datacenter is, not only in Europe: your files leave Europe for this run. |
| `--lane <name>` | A lane of its own on your machine, where its build output stays warm between runs. |
| `--env NAME[=value]` | Sends this environment variable, beside those of `.hangar/cloud.env`; none are sent otherwise. |
| `--accept-use-policy` | Accepts the acceptable use policy, once, before a first run. |

`hangar-ai cloud forget-cache`, in the project's folder, forgets its dependency cache
(below).

`Ctrl`+`C` stops the job and prints what it cost; a second one leaves at once. The exit
code is the command's; otherwise 75 when no machine could be had, 77 for not enough
credits or another refusal (each with a sentence saying what to do), 124 at the time
limit, 130 when stopped.

**Delivery checks.** In the orchestration panel, under the check command, **Run delivery
checks on** says *This computer* or *HangarAI Cloud*. In the cloud, each delivery is
merged on your computer as always, then its check runs on your machine, in a lane of the
workspace's own; deliveries checked together share one run, as they do here. The machine
is kept 5 minutes after a check, for the next delivery. The panel shows what a check
costs on average, on a machine already up and on one rented for it.

## Choose a machine

Each machine comes from a **class**, which says what it is for, what it has and what it
costs now. The price sheet shows them as cards with their rate and how many are free
right now; `hangar-ai cloud classes` lists the same.

| Class | For | What it has | Your limit |
|---|---|---|---|
| CPU · 32 threads (`threads_32`) | Builds and tests | 32 threads, 63 GB or more | 12.5 credits a minute |
| CPU · 64 threads (`threads_64`) | Builds and tests | 64 threads, 125 GB or more | 25 |
| GPU · 1× RTX 4090 (`gpu_rtx_4090`) | ML training and inference, rendering | 1 RTX 4090 (24 GB), 8 threads, 32 GB or more | 15 |
| GPU · 1× A100 80 GB or H100 (`gpu_a100_h100`) | ML training and inference | 1 A100 80 GB or H100 (80 GB), 16 threads, 64 GB or more | 60 |
| Multi-GPU · 2 to 8 GPUs (`gpu_multi`) | ML training, rendering | 2 to 8 GPUs of 24 GB or more each, 32 threads, 128 GB or more | 200 |
| VM · 1× RTX GPU, own kernel (`vm_rtx`) | What a container cannot run: perf, bpftrace, userfaultfd, game sessions | A **full VM**: its own Linux 6.8, root; 1 RTX 4090, 5090, PRO 5000, 6000 Ada or PRO 6000 (24 GB or more), 8 threads, 32 GB or more, 300 GB | 25 |

The classes are HangarAI's catalog and may grow; the sheet and `hangar-ai cloud classes`
always show the current one. Pick the smallest that fits: a GPU is for work that uses
one (CUDA, PyTorch, Blender…); a build or a test suite runs faster for less on CPU
threads.

**Advanced.** Rather than a class, say what the machine must have, each part "at least":
the number of GPUs (exactly) and their models (or any), the memory of each GPU, CPU
threads, memory and disk. The sheet shows the machine it found and its rate. When
nothing free matches, it shows the **closest offers** instead, each with what it misses
(a different GPU model, less GPU memory, fewer threads…), so you can loosen the spec or
pick one of them.

**Region.** Europe first, always: a run leaves Europe only when you choose **Any region**
for it.

### Full VMs

The other machines are **containers**: fast to start and cheap, but their kernel is their
host's and their rights a container's. Some work needs more than that: **perf**,
**bpftrace**, `userfaultfd` (with `WP_ASYNC` and `PAGEMAP_SCAN`, Linux 6.7 or later), gdb
attached to a running process, a reservation of a terabyte (`MAP_NORESERVE`), `mprotect`
by the hundred thousand a second. A **full VM** gives it: a virtual machine of its own,
with its own kernel, rented from the same datacenters and billed by the second like the
others.

- **What you get**: Linux 6.8, NVIDIA's driver with the GPU passed through, and your run in
  the same box as on a container (your files, your output, your data and caches, a time
  limit, the machine kept or destroyed). perf works for your command as it is (its
  software events, `perf record` with the kernel's symbols); `userfaultfd` too. The CPU's
  **hardware counters** (cycles, cache misses) are usually not there: the hosts do not
  expose them to their VMs.
- **Root**, with `sudo`, on the VM class, or with `--root`: bpftrace needs it, and gdb to
  attach to a process your command did not start. Root can do anything on its machine,
  HangarAI's box on it included, so a VM with root whose box stops answering bills your run
  for what it used, rather than giving it back and running it again as for any other
  lost machine. Without root, a VM's command is held as on a container.
- **What it costs**: its machine's rate, as any (its dollars an hour × 16.5 credits a
  minute). On 21 November the VMs of certified datacenters in Europe started at 0.89 $ an
  hour (an RTX PRO 5000) and 1.02 $ (an RTX 5090), plus about 0.09 $ an hour for the
  class's 320 GB of disk: about 16 to 18 credits a minute. A 15-minute session on a VM
  already up is about 260 credits, 0.52 € at the 5,000 pack's price. A VM takes about **2 minutes** to start (a container, seconds to one),
  which you pay as the start of any run, and fetching a big game costs its host's
  traffic (128 GB at 0.0117 $ a GB, about 1,480 credits): keep the machine between the
  runs of a comparison (a group) rather than fetching it each time.
- **When to use it**: only when a container cannot run the work. Builds, tests and GPU
  work that needs no kernel of its own are faster and cheaper on the other classes.
- **How**: the `vm_rtx` class in the sheet or with `--class vm_rtx`; or said part by part:
  `--vm`, `--kernel 6.7` (the least kernel; a VM runs 6.8 today, a newer one is refused
  with a sentence), `--root`, beside `--gpu`, `--cpus`… A VM runs its own system: it
  takes no image of yours (`--image` is for a container).

```sh
hangar-ai cloud run --class vm_rtx --data games:yotei -- ./bench.sh --perf
hangar-ai cloud run --gpu 4090 --kernel 6.7 --root -- sudo bpftrace -e 'tracepoint:syscalls:sys_enter_mprotect { @ = count(); }' -c ./game
```

**In the sheet**, a class your license cannot rent stays in its place, greyed, with why.
**Advanced** starts from the class
you were on. The machine you run on is the project's from then on: its next sheet opens
with it. A run typed in a terminal names its own (32 threads unless it says), and
`hangar-ai cloud classes` prints the catalog with its rates and free machines now, the GPU
models `--gpu` takes and whether your license may rent a GPU (`--any-region`: anywhere).

**Agents** can propose a machine too: they say what they need and why, and you approve
it, unless it fits in the workspace's daily budget ([Machines for agents](#machines-for-agents)).

**With a GPU.** A run of a GPU class or spec gets the machine's GPUs: their devices, and
`CUDA_VISIBLE_DEVICES` set to them, for its command alone. A CPU run never reaches the
GPU a machine may carry. GPU machines are for **Pro and Max** licenses, like
every machine, and they count in your machines at once (3, or 5 with Max, your hosted nodes
among them). There is no
limit of hours, on a GPU or a CPU: a machine runs as long as your credits pay for it.

## What happens to a run

| State | What you see | Billed |
|---|---|---|
| Queued | Your place and the wait; Cancel | no |
| Waiting for a machine | No machine under your limit is free yet; "Raise the limit" for this run. After 10 minutes it gives up. | no |
| Starting | Renting a machine · Setting up · Restoring the cache, then the machine (processor, threads, memory, country, rate) | yes |
| Running | The output, as in any terminal; elapsed time and credits so far; Stop | yes |
| Kept | A countdown, "Destroy now", "Keep 10 min more" | yes, at the same rate |
| Destroyed | The receipt: start, run, kept, cache, and what was refunded | |

- **Your machine serves your next runs.** While it is up, a new run goes to one of its
  four lanes at once, with no start to pay. Pro has up to 3 machines at
  once, Max 5.
- **The run lives with its connection.** Closing the terminal, or losing the network for
  good, stops the job.
- **A machine that fails is not charged.** If the host fails or stops answering, what
  the run used is refunded and it is tried once more on a new machine.
- **Logs.** The Cloud space's row has Logs, Stop and Destroy; its receipt, beside the
  list, has **Forget cache** too, for the run's project. Your computer keeps the output of
  your last 50 runs (64 MB each at most) in the app's data folder, under `cloud-builds/`.

## What is sent

What `git ls-files` lists, from the repository's top:

- **the files git tracks**, except those named like a secret: `.env` and `.env.*`,
  `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `id_rsa` and the other SSH
  keys, `credentials` and `credentials.*`, `secrets.*`, `.netrc`, and anything in `.ssh`,
  `.aws` or `.gnupg` at the top. A source file whose name only contains `token` or
  `secret` (`src/token.rs`) is sent;
- **untracked files that git does not ignore**, except those in a tool's folder at the
  top (`.claude`, `.hangar`, `.vscode`, `.idea`, `.ssh`, `.aws`, `.config`…), those named
  like a secret (the names above, and anything with `secret`, `token` or `credential` in
  its name) and those over 16 MB;
- **never** `.git` nor a file git ignores.

What stays on your computer is said: in the sheet, and on the run's first lines.

**Sending one anyway.** `.hangar/cloud-include`, in the workspace's folder, lists the
files a run sends although they are named like a secret or sit in a tool's folder, one
path or glob a line (`*` within a name, `**` across folders, `#` for a comment):

```
# .hangar/cloud-include
.env.example
config/test-*.pem
```

**Environment variables** go only when the project lists them, or when you pass them with
`--env`. `.hangar/cloud.env` lists them, one a line: `NAME` sends the value it has where
the run starts (your terminal's, or the app's for a delivery check), `NAME=value` that
value. A name that is not set is said and left out. Their values go to the machine over
SSH for the command alone: never in the run's output kept on your computer, its logs,
the command line typed into a pane, nor to HangarAI's server; the sheet and the run show
their names only.

```
# .hangar/cloud.env
NPM_TOKEN
RUST_LOG=info
```

`.hangar/` stays out of git and is never sent (a worktree's runs read the main
checkout's), and `.hangar/cloud.env` never leaves your computer as a file, whatever
`cloud-include` says.

Only the files that changed since the last run on that lane are sent again. The
dependency cache (cargo, npm, pnpm, pip, uv, Go, Gradle and the build output of your
dependencies, never your own code) is restored before the command and saved after it,
encrypted on the machine with a key that stays in your computer's keychain. A computer
without one (a server, Linux reached over SSH or without a desktop session) keeps the key in
a file of the app's data folder, `keys/`, that only your user can read and that nothing
syncs or sends; the run that first puts it there says so.

**Forget this project's cache**: **Forget it** in the sheet, **Forget cache** in a
machine's receipt in the Cloud space, or `hangar-ai cloud forget-cache` in the project's
folder. Its encrypted copies on HangarAI Cloud are deleted at once, and its key, in your
keychain or its file: a copy a machine still kept would save afterwards can never be opened.
Its next run starts cold.

## Big projects: data, caches, results, your image, groups

A run sends the files git lists. A big project needs more: a game of 200 GB that git
ignores, a dataset, the saves a benchmark starts from; a compiler cache of 40 GB and
shader caches that take minutes to build again; captures, logs and traces that come back;
a toolchain or a driver of its own; and, for a comparison, both runs on the same GPU.

### Project data

```
hangar-ai cloud data add games Build/games/yotei Build/games/stray
hangar-ai cloud data sync games
hangar-ai cloud data ls
hangar-ai cloud data rm games
```

- **`data add <name> <path>…`** makes a *set* of the files and folders you name, any of
  them: ignored by git, huge, outside the project. A path in the project goes to the same
  place in each run's copy; one outside it goes to the copy's top, under its own name.
  The set is read, then **what keeping it costs is said before anything is sent**:
  *204 GB: about 3 060 credits a month (about 2.88 €), kept until you delete it; 128 GB to
  send, 76 GB the storage has already*. You say yes (or pass `--yes` in a script);
  nothing is sent otherwise.
- **`data sync [<name>]`** sends what changed, in one set or all of them. A set is one
  stream of its files, cut where their content says into chunks of 4 to 64 MiB: a file
  that did not change (its size and time of change) is not even read again, and a game
  relinked sends its executable's few changed chunks, not the game. Chunks the storage
  holds already, from this set or another of the project, are never sent again. A sync
  cut short (the line, Ctrl+C) goes on where it stopped. Eight chunks go at once, and the
  progress is said: *12 GB of 128 GB sent · 85 MB/s · ≈ 22 min left*.
- **`data ls`** lists the project's sets: their size, files, chunks, what they store,
  what that costs a month, when they were sent, and whether they changed here since; then
  what all your stored data costs a month and what it cost so far this month.
- **`data rm <name>`** deletes a set from the storage, after asking: it is no longer
  billed from the next day.

**Sealed here.** Every chunk is compressed, then sealed on your computer with the
project's **data key** (AES-256-GCM, a key of its own per chunk), and named by an HMAC of
its bytes under that key: HangarAI's storage holds ciphertext and ids that say nothing of
your files, and a set's name is sealed too. The key is kept in your keychain beside the
cache's (in a file of the app's data folder, yours alone, on a computer without one), and
goes to a run's machine over SSH for the run, never to HangarAI's server. Lose it and
what was sent with it cannot be opened: on a computer without a keychain, keep a copy of
its file. A file named like a secret (`.env`, `*.pem`, `id_rsa`…) is left out and said,
unless `.hangar/cloud-include` lists it. Symbolic links go as links.

**Cost.** Stored data is billed in credits by the GB-month (about 15 credits a GB-month,
R2's price), a little each day, each chunk once however many sets hold it; reading it
back from a run is free, and a run never sends it again. On the first of each month,
HangarAI says what your storage cost the month before and costs now. Nothing is deleted
until you delete it; a month the credits do not cover stays due, and uploads wait for it.

A project's sets are its main checkout's: a worktree's runs use them too.

### A run that takes data, caches and results

```
hangar-ai cloud run --gpu rtx_4080 --data games:Build/games/yotei \
  --cache 'pipelines/{gpu_model}/{driver}=.cache/kyty' --cache ccache \
  --results captures -- ./bench yotei --minutes 10
```

- **`--data <set>[:<paths>]`** gives the run a set, all of it or only these of its paths
  (with commas), in their place in the copy: the machine fetches them from the storage
  before the command (a machine kept for your next run keeps what it fetched). A set not
  sent yet stops the run before anything is spent; one that changed here since it was sent
  is said, and the run gets the version sent.
- **`--cache <name>[=<dir>]`** restores a persistent cache into `<dir>` (in the machine's
  home, or absolute) before the command and saves it after, whatever the exit code.
  `ccache`, `sccache`, `pip`, `uv`, `huggingface`, `torch`, `nvidia` and `mesa` know their
  folder. `{gpu_model}` and `{driver}` in a name are the machine's, so a shader or pipeline
  cache is kept per GPU model and driver. A cache is sealed with the project's data key,
  billed as data, and kept 30 days after a run last used it.
- **`--results <dir>`**: what the command leaves in `$HGC_RESULTS`, and the files of
  `<dir>` (a folder of the copy, from where the command runs), come back when it ends,
  into `.hangar/cloud-results/<job>/` in the workspace (or `--results-to <dir>`), opened
  here, and are said with their size: *results: 214 files, 48 MB in
  .hangar/cloud-results/j_4b1e/*. They then leave the storage. Results that could not come
  back stay there 30 days: `hangar-ai cloud results [<job>] [--to <dir>]` fetches them.
- **`--image <ref>`** starts the machine from your image (any Linux image with a shell,
  `ubuntu:24.04`, `ghcr.io/you/ci:1`), HangarAI's box put in it at start: a toolchain that
  matches yours, a driver. HangarAI's own image is the default. From a terminal and an
  agent, a public image (a registry that asks no sign-in); a private registry's sign-in is
  kept, sealed, in the app's Cloud settings.
- **`--group <name>`** puts runs on the same GPU model and host, the machine kept warm with
  their data between them: both arms of an A/B measure your change, not two machines.
- **`--priority high|normal|low`** orders your own queued runs.

**`.hangar/cloud.toml`** gives a project's defaults, which a run's options replace:

```
# .hangar/cloud.toml
data = ["games:Build/games/yotei", "saves"]
caches = ["ccache", "pipelines/{gpu_model}/{driver}=.cache/kyty"]
image = "ghcr.io/you/kyty-ci:latest"
results = "captures"
```

A line it cannot read is said by its number, and left out.

**Agents** take the same: `cloud_run` and `cloud_suggest` have `data`, `caches`,
`results`, `image`, `group` and `priority`, and `cloud_data` lists the project's sets.
`cloud_suggest` asks for the disk a run's data takes. A game session on a GPU is
`data ["games:Build/games/yotei"]`, a pipeline cache, `results "captures"`; an A/B is two
runs with the same `group`. Only you add, sync or delete a set: an agent asks you.

## Prices

Cloud Builds is paid with your license's credits, at a rate shown before the run starts.
From HangarAI's credits configuration:

- **Credits**: Pro includes 3,000 a month, Max 10,000 a month. Packs
  of 1,000 for 3 € and 5,000 for 10 €. Pay as you go, if you turn it on, at 3 € per 1,000
  credits up to a monthly limit you choose (10 € unless you change it, 200 € at most).
  Prices include VAT; the plans are on the [pricing page](https://hangarai.dev/pricing/).
- **A machine's rate is its own**: machines are rented on a marketplace where each has its
  price, so the rate is **its price in dollars an hour × 16.5 credits a minute**, and its
  traffic **its price in dollars a GB × 989 credits a GB**, for a GPU machine as for any.
  It is fixed when the machine is rented, until it is destroyed. On 19 November 2026 the
  cheapest 32-thread machine in Europe cost 0.431 $ an hour: **7.1 credits a minute**,
  426 an hour, about 0.85 € at the 5,000 pack's price. An RTX 4090 machine at 0.50 $ an
  hour is 8.25 credits a minute; an H100 at 2.40 $ an hour, 39.6.
- **Your limit** on the rate is the class's (the table above): no dearer machine is
  rented, and you can raise it for a run, up to 1,000 credits a minute. For an advanced
  spec, the sheet proposes one and a half times the best rate it found.
- **What is billed**, by the second: the start (renting, setting up, restoring the
  cache), the run, the minutes the machine is kept, and the traffic its host bills. Runs
  side by side on one machine share its time.

For example, on a 7.1 machine: a 4-minute check costs 28.4 credits on a machine already
up. From nothing, add about 2 minutes of start, the cache's traffic (about 19 credits for
1.5 GB on a host at 0.013 $ a GB) and the 2 minutes kept: about 76 credits.

**What a run holds.** Before it starts, a run holds the most it can cost: your limit ×
(its time limit + 3 minutes of start + its keep time), plus 30 credits of traffic. That
is 467.5 credits for 30 minutes on 32 threads. It is charged what it used, never more,
and the rest is given back when it ends. A run cancelled in the queue, one that found no
machine, and one whose machine failed cost nothing.

## Limits

| | |
|---|---|
| Machines | A class of the catalog, or an advanced spec: up to 8 GPUs, 512 threads, 4 TB of memory and 4 TB of disk. Certified (ISO 27001) datacenters, in the European Union and EEA, Switzerland, the United Kingdom or Serbia; anywhere such a datacenter is for a run you chose **Any region** for. A run of Europe never goes to a machine of yours elsewhere. A run goes only to a machine of its own class or spec. |
| Full VMs | Linux 6.8; root only on the VM class or with `--root`; about 2 minutes to start, 10 at most before the machine is given up and the run tried again; no image of yours. |
| Time limit | 30 minutes by default, 2 hours on a machine with a GPU; 1 minute to 24 hours. The cache's restore counts in it. |
| Kept after a run | The project's setting: 2 minutes unless the sheet chose none, 5, 10 or 30 (5 for delivery checks); 30 at most, or less if HangarAI's server says so. |
| At once | 4 runs side by side on a machine; Pro 3 machines, Max 5, GPU machines and [hosted nodes](hosted-nodes.md) among them: with 2 hosted nodes running, a Pro license has 1 build machine. |
| The queue | Max first, then Pro, first come first served within each; a waiting run moves up a class every 10 minutes, so Pro never waits more than 10 minutes behind Max. |
| Hours | No limit of hours a day, on a CPU or a GPU: your credits are the limit. A machine itself lives 29 h 20 at most: a run that would end less than an hour before that goes to a new machine instead, and a machine is never kept past it, so no run is ever cut, a day-long one included. |
| Traffic | 20 GB out of the machine per run. |

## When the credits run out

- **Before a run**: if what it holds does not fit in your credits (pay as you go
  included, up to your limit), it does not start and nothing is charged. The sheet offers
  the longest time limit that fits, and Buy credits; the command says the same, with the
  `--time-limit` that fits.
- **During a run**: never stopped for credits. What it can cost was held before it
  started.
- **After a run**: the machine is kept only if its keep time fits in your credits
  without pay as you go; otherwise it is destroyed at once. The same for "Keep 10 min
  more". A machine is never kept on pay as you go.
- **Delivery checks in the cloud**: under the setting, **When the credits run out** says
  *Check here* (the default: the check runs on this computer, and HangarAI says so once)
  or *Wait for credits* (deliveries wait, up to their check's time limit, and nothing is
  checked here).

One other reason a run cannot start, said with its date and costing nothing: HangarAI's
monthly limit with its machine provider is reached, and Cloud Builds pause for everyone
until the month ends.

## Machines for agents

Claude Code and Codex in Hangar have four tools to pick a machine, run on it and use your project data (the
[MCP guide](mcp.md#cloud-machines) has their details):

- `cloud_classes` lists the catalog as the terminal does;
- `cloud_suggest` takes what the work runs (and what the agent knows of it: the GPU, its
  memory, the threads, the memory, the disk, how long) and answers the smallest machine
  that fits, its rate, what the work likely costs and the most it can, and why. It asks
  for a GPU only when the work runs on one: a CUDA training job gets the RTX 4090, a
  `cargo test` 32 threads; and for a [full VM](#full-vms) only when a container cannot run
  the work (perf, bpftrace, userfaultfd, gdb on a running process, a kernel version), root
  only when it needs it (bpftrace, gdb attaching);
- `cloud_run` runs a command on the machine chosen, from the agent's folder, as
  `hangar-ai cloud run` does. Within your workspace's daily budget for agents (100
  credits unless you set another, Settings › Cloud) it starts at once; past it,
  **Needs you** asks you first, with the machine, the command and the most it can cost:
  *Allow once*, *Allow more today* or *Deny*. The agent follows what it prints and gets
  its receipt at the end; the run is a row of the Cloud space like yours, where you can
  stop it. The acceptable use policy is yours to accept: an agent never does.
- `cloud_data` lists your project's data sets, so that a run can name them
  ([Big projects](#big-projects-data-caches-results-your-image-groups)).

## Privacy and acceptable use

- Your files and the output travel over SSH between your computer and the machine,
  whose host key HangarAI checks for you before the first connection. HangarAI's server
  never sees a file, the command or a line of output.
- The command runs as an ordinary user, not root, on a machine used by you alone. While
  it runs, the machine watches for abuse (programs' names and fingerprints, their use of
  the processor and the graphics card, which of them hold the graphics card, outgoing
  connections' ports and counts), never your files or your commands.
- When the machine goes, your files, its keys and its cache are deleted, then the
  machine and its disk are destroyed. The encrypted cache is kept 14 days after each
  save, unless you forget it sooner.
- The machine is in Europe unless you chose **Any region** for a run; then it may be in
  any country with a certified datacenter, and your files go there for that run.
- No mining, proxies, scanning, attacks or mail: a run that does is stopped, not
  refunded, and mining freezes Cloud Builds on the license until a review. On a GPU, a
  known miner (by its name or its fingerprint), a connection to a mining pool, a GPU held
  full for 10 minutes with a miner's still memory by something that is not your
  command, or held full for 10 minutes while a connection to an unusual port stays open
  the whole time, is mining.

The details are on the [privacy page](https://hangarai.dev/privacy.html#cloud-builds) and
in the [HangarAI Cloud terms](https://hangarai.dev/terms/#cloud-builds). Running checks on
your own computer stays the default.
