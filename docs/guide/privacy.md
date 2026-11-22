# Privacy

[← Hangar](../../README.md) · [Guide](README.md)

Hangar has no usage telemetry. Your workspaces, the board, the memory and the statistics
stay on your machine:

- each workspace's board is in `.hangar/board.json` in the project. Hangar keeps it out of
  `git status` through `.git/info/exclude`, unless you commit it yourself;
- the app state and the memory are in the app's data folder;
- the statistics are computed from the transcripts the agents already keep locally, and
  are never sent anywhere unless you turn on the optional statistics sync of the Hangar
  account;
- the MCP API listens on `127.0.0.1` only, behind a token that changes at each launch;
- [Hangar Mobile](mobile.md), off until you turn it on, listens on `127.0.0.1` too, and
  only your own devices reach it, through Tailscale or, if you turn it on, through the
  end-to-end encrypted relay described below.

HangarAI itself only goes online in these cases:

- **The license.** HangarAI is paid from 3.0, after a 14-day trial; installs of 2.5.1 and
  earlier get a free license. The app checks its license with HangarAI's account server
  (`account.hangarai.dev`), and it sends nothing about how you use it. What it sends:
  - **at the first launch**, to start the trial or get the free license (both after you
    sign in to a HangarAI account): a hash of the machine and the app's version. The hash is an HMAC-SHA256 of the identifier your
    operating system keeps for the machine; the identifier itself never leaves the PC. A
    trial is started from your HangarAI account: you sign in to it in your browser, with
    GitHub, and the server notes that the account had its trial;
  - **with each activation and renewal**, a second signal: an HMAC of what the system says
    of the computer (its name, model and processor; on a Mac, its serial number and model),
    keyed by the machine's identifier, so
    the server can neither tell what the computer is nor link two machines. The server
    counts, per seat and per day, how many computers and addresses use a license, to see a
    license copied to many computers, and forgets them the next day;
  - **about once a day** while the app runs and the server can be reached, to renew the
    license: the license's signed token, which holds only what the server gave it (the
    license's id and kind, the machine's hash, its dates), and the app's version. Offline, the app keeps working for two weeks after it last reached
    the server;
  - **when you type a license key**: the key, the machine's hash and the app's version;
  - **when you deactivate this machine**: the token, to free its seat;
  - **when you are signed in to the Hangar account**: the token, with the account's, so
    that the account knows this machine's license;
  - **when the license on this PC fails its checks** (a token altered, signed by another
    key, copied from another machine, or a clock set back): a report with the license's
    id when there is one, the app's version and a reason code, and nothing else. The
    server keeps reports 180 days.
- **Credits and purchases.** HangarAI's hosted features are paid with credits. Settings ›
  License reads the balance from the account server with the license's token. To buy, the
  app asks the server for a checkout (the license's id or the machine's hash, the offer,
  and a referral code if you typed one), then opens it in your browser. The payment is
  handled by [Polar](https://polar.sh), the merchant of record, under its own privacy
  policy. The server counts how many credits each hosted request used, never what it
  processed.
- **Dictation.** The audio you record is sent to HangarAI's account server, which has
  [Groq](https://groq.com) transcribe it and sends the text back. Groq receives the audio
  only: nothing in it identifies your license, your machine or you, and the audio file's
  metadata is removed first. Neither the server nor Hangar keeps the audio or the text; the
  server writes down only the license's id, how many seconds were billed and whether it
  worked.
- **Image generation**, when an agent draws or edits an image for you: the prompt, and the
  image to edit, go through HangarAI's server to [fal](https://fal.ai), in the United
  States, which is told not to keep them. The image comes back to your computer; the server
  writes down only the license's id, the credits it took and how many images it drew
  that day.
- **Share links**, when you or an agent publish one, after your OK: the files go to
  HangarAI's storage in the EU (Cloudflare R2) and stay there until the link expires, then
  are deleted. Anyone with the link can open them. A replay is published the same way, once
  you have seen what goes.
- **Cloud Builds**, when you run something in the cloud or a workspace checks its deliveries
  there: the project's files go to a machine rented in Europe for your job alone, a CPU one or
  one with a GPU (Vast.ai's Secure Cloud, certified datacenters), and are wiped with it;
  elsewhere only for a run you send there. The cache of your dependencies is encrypted and
  kept 14 days ([Cloud Builds](cloud.md)).
- **Hosted nodes**, when you keep a workspace running in the cloud: your workspace goes over
  SSH, straight from your computer, to a server rented for you alone at Hetzner, in Falkenstein,
  Nuremberg or Helsinki. HangarAI's server holds the session's record, never your code. At the
  end, the work is brought back over SSH, or parked in Cloudflare R2 in the EU, sealed with a
  key only your computer holds, and deleted once restored, or after 7 days
  ([Hosted nodes](hosted-nodes.md)).
- **Hangar Mobile from anywhere** (paid licenses and the trial, off unless you turn it on).
  Your PC and your phones connect to a relay on HangarAI's account server, which passes their
  traffic along. That traffic is encrypted end to end between your devices, with keys the
  server never has: the relay cannot read your projects, messages or files. It sees when your
  devices connect and disconnect, the size and timing of what they exchange, the IP addresses
  they connect from, and the license your PC uses. It stores none of this: nothing is written
  to disk or to logs, and it is gone when the connection closes. A phone that pairs through
  the relay without Tailscale runs the Hangar Mobile web app the server serves: the encryption
  protects your data from the relay, but you rely on that app being the published one. To keep
  the server out of the picture entirely, pair over Tailscale.
- **Your computers** (paid licenses and the trial, on by default; *Show to my other
  computers* in Settings › Computers turns it off). Each computer with HangarAI open keeps a
  connection to the same relay, so that your other computers on the license can list it: the
  relay sees each computer's name and system and when it is connected, and tells them to
  your license's other computers and nobody else. What two computers say to each other is
  encrypted end to end with keys they agree on when you pair them with a six-digit code: the
  relay sees its size and timing, and the IP addresses they connect from. It keeps none of it
  ([Your computers](computers.md)).
- **Updates.** It checks the GitHub release feed for a new version, and installs one only
  when the feed and the build are signed with HangarAI's release key.
- **Discord.** Rich Presence talks to your local Discord client, and only if you turn it
  on. HangarAI on Discord, once you set it up in Settings › Discord, posts what your
  orchestrators and workers do and say, their cards and the files agents share to your
  Discord server, through Discord, which can read them. The bot's token stays in HangarAI's
  data folder, readable by you alone.
- **Hangar Mobile notifications.** Once you turn them on on a phone, the PC sends them
  through that phone's push service (Apple's, Google's or Mozilla's), encrypted for that
  phone alone, so the push service cannot read them.

The [privacy page](https://hangarai.dev/privacy.html) says what the account server stores
and how long. The agents you run talk to their own providers under their own settings.
Hangar does not change that.
