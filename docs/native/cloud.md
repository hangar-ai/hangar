# The Hangar account (optional)

A Hangar account ("Compte Hangar") links several machines: sign in with GitHub
on each, and the usage statistics of all of them can be read from any of them.

It is optional and off by default. A Hangar nobody signed in with never calls
the account server, has no code path waiting on it, and works exactly as it
did before the account existed. Nothing in the app requires one, now or later.

Not to be confused with the *Claude accounts* a pane runs under
(`crates/hangar-core/src/accounts.rs`): those are Claude Code's logins, kept on
the PC, and have nothing to do with this.

## Pieces

| Piece | Where | What |
|---|---|---|
| Protocol types | `crates/hangar-cloud-api` | Requests, answers, paths, limits, `API_VERSION`. Serde only; both sides build from it. |
| Server | `crates/hangar-cloud` | One binary (axum, SQLite through rusqlite's `bundled`), one database file, a Docker image behind Cloudflare at `https://account.hangarai.dev`. |
| Desktop | `crates/hangar-core/src/cloud/` | Sign-in, the device token in the OS keychain, the sync. Behind hangar-core's `desktop` feature: a node has no browser to sign in with. |

The desktop reaches `https://account.hangarai.dev`, or the address in
`HANGAR_CLOUD_URL` when that variable is set (a local server, while developing).

## Privacy

### What leaves the PC

Nothing, until the person signs in.

At sign-in, once: the machine's name, its operating system (`windows`, `macos`
or `linux`) and an id the desktop drew at random for this install.

After that, **only while the statistics sync is on** (a setting of its own,
which signing in does not turn on by itself), totals per machine, day, model
and project name:

- the day (`YYYY-MM-DD`, in the machine's time zone), the model's id, and the
  project's **name**: the last component of its folder (`hangar` for
  `C:\Users\ada\Projects\hangar`);
- input, output, cache-read and cache-write tokens;
- what the API would have charged for them;
- active seconds, and the number of sessions.

Never a path, never a session's title, never a prompt, an answer or any other
line of a transcript. `StatsRow` has no field for them, and the server refuses
a project that holds a path separator, so a mistake on the desktop cannot send
one.

### What the server stores

- **The account**: an id, and what GitHub said at the last sign-in: a
  display name, an email address when GitHub gives a verified one (kept in
  lower case), an avatar URL when it is an `https` one, and GitHub's own id
  for the person. GitHub is asked for `read:user user:email`. Its access
  token is used once, to read that profile, and is not kept. An address
  GitHub does not vouch for is not stored at all.
- **The devices**: for each machine, the server's id, the machine's id, name
  and operating system, when it was linked and last seen, and the SHA-256 of
  its token. The token itself is sent once, at the exchange, and exists
  afterwards only in the machine's keychain.
- **The statistics rows** each device sent.
- **Sign-ins in progress**: what `/v1/auth/start` was given (the port, the
  desktop's state and challenge, the machine's name) with the server's own
  state (as its SHA-256) and PKCE verifier, until the provider sends the
  browser back and for ten minutes at most; then the link code (as its
  SHA-256), for five minutes at most.

The server keeps no request log of its own and stores no IP address: the rate
limits count addresses in memory, for a minute. Cloudflare, in front of it,
sees what any reverse proxy sees.

What a person is told of all this is the site's privacy page,
`site/privacy.html` (https://hangarai.dev/privacy.html), which the Hangar
account section of the settings links to. When this section changes, that page
changes with it.

### Deleting

- **Signing out** (`DELETE /v1/devices/{id}` on oneself) destroys the device's
  token. Its rows stay on the account, so that signing in again from the same
  machine finds its history where it was.
- **Forgetting a machine** (`DELETE /v1/devices/{id}?forget=true`) deletes the
  device and every row it sent.
- **Deleting the account** (`DELETE /v1/account`) deletes everything: the
  account, its identities, its devices and their tokens, every row. At once,
  with nothing kept aside. The desktop then forgets its token too.

On the PC, the token is in the operating system's keychain (Credential Manager,
the macOS Keychain, the Secret Service), like the Groq key
(`crates/hangar-core/src/voice/key.rs`), and nowhere else. When no keychain
answers, sign-in fails and says so: the token is not written to a file instead.
With `HANGAR_CLOUD_URL` set, the keychain entry is `hangar-account@<url>`, one
per server: a development server is never sent the real server's token.

## Signing in

The server runs the OAuth code flow with the provider; the desktop never sees
a provider's token or secret. Between the desktop and the server there is a
second, independent PKCE and state, so that only the Hangar that started a
sign-in can finish it.

```
desktop                          browser                 server                  provider
   |  listen on 127.0.0.1:<port>    |                       |                        |
   |  draw state + verifier         |                       |                        |
   |-- open /v1/auth/start -------->|---------------------->|                        |
   |                                |                       | store port, state,     |
   |                                |                       | challenge; draw its    |
   |                                |                       | own state + PKCE       |
   |                                |<-- 302 authorize -----|                        |
   |                                |------------------------------- consent ------->|
   |                                |<------------ 302 /v1/auth/callback/<provider> -|
   |                                |---------------------->| code -> token ->       |
   |                                |                       | profile (once)         |
   |                                |<-- 302 127.0.0.1 -----| draw link code         |
   |<- GET /callback?code&state ----|                       |                        |
   |  check state                   |                       |                        |
   |-- POST /v1/auth/exchange {code, verifier, machine} --->| check code, S256       |
   |<-- {token, account, device} ---------------------------|                        |
   |  token -> keychain             |                       |                        |
```

- The desktop's **verifier** is 43 to 128 characters of `[A-Za-z0-9-._~]`
  (RFC 7636), drawn from the OS's random source; the **challenge** is
  `base64url(sha256(verifier))` without padding. The **state** is random too:
  1 to 200 characters of `[A-Za-z0-9-._~]`, which is what base64url and
  hexadecimal are made of. The server refuses any other, so that the state
  comes back in the loopback's URL exactly as it left.
- The server draws **its own state and PKCE verifier** for the provider (32
  random bytes each, in base64url). The provider never sees the desktop's
  state or challenge, and the desktop never sees the server's. The server's
  state is stored as its SHA-256, works once, and for ten minutes: the
  callback deletes it as it reads it, before anything else.
- The browser is only ever sent to the provider's consent page, or back to
  `http://127.0.0.1:<port>/callback` with the port the start stored, from 1024
  up: the host and the path are the server's, never the request's. Nothing
  else in a request (a `redirect_uri`, a `Host`, a second `port`) is ever
  redirected to.
- A sign-in may spend **ten minutes** at the provider, which is how long the
  desktop's listener waits (`SIGN_IN_TIMEOUT`). After that the server's state
  is unknown and the callback shows a page instead of redirecting.
- The **link code** is single-use and expires five minutes after the callback
  drew it. It is useless without the verifier, which never went through the
  browser.
- The desktop's listener answers a callback whose `state` is not its own with
  a `400` page and **keeps waiting**: a stray or forged request to the
  loopback port does not end the sign-in in progress.
- The **device token** is `hgr_` followed by 32 random bytes in base64url. It
  does not expire; it ends when the device signs out, is forgotten, or the
  account is deleted.
- **One device per machine.** The exchange carries the machine's id: when the
  account already has a device with that id, it is that device that is signed
  in again (its previous token stops working, its rows are kept) rather than a
  second one that appears.
- **One account per identity.** An identity (a provider and its id for the
  person) belongs to one account. At each sign-in the server looks for the
  account this identity already belongs to (`account::resolve`): its name,
  address and avatar become what GitHub says today, and an address GitHub no
  longer vouches for is forgotten. Otherwise it creates a new account. An
  address never finds an account: the same address at another GitHub
  identity is another account. The address is the one of `/user/emails` that
  is both primary and verified (never the public `email` of `/user`).

## The protocol

Everything is under `/v1` (`API_VERSION`), over HTTPS, with JSON bodies in
UTF-8. `crates/hangar-cloud-api` is the reference: each request and answer
below is a type there, each path a constant of its `paths` module. Times are
milliseconds since the Unix epoch.

### Errors

Every answer that is not a success is

```json
{ "error": "invalid_code", "message": "the link code is unknown, used or expired" }
```

with the status of its code. `message` is English, for logs and bug reports;
the desktop words what it shows from `error`.

| `error` | Status | When |
|---|---|---|
| `bad_request` | 400 | A field missing or malformed, a name too long, a day that is not a day. |
| `invalid_code` | 400 | The link code is unknown, used or expired, or the verifier does not match. Which is not said. |
| `unauthorized` | 401 | No device token, or an unknown or revoked one. The desktop forgets its token and shows the account as signed out. |
| `not_found` | 404 | No such device on this account; no such route. |
| `method_not_allowed` | 405 | The route does not take this method. |
| `too_large` | 413 | More than 5000 rows, or a body over 4 MiB. |
| `rate_limited` | 429 | Too many requests; `Retry-After` says when to come back. |
| `internal` | 500 | The server's fault. |
| `provider_error` | 502 | GitHub refused, or answered nonsense. |
| `provider_unavailable` | 503 | This server has no OAuth client for the provider. |

A desktop reads a code it does not know as a refusal like any other
(`ErrorCode::Unknown`).

### Without a token

#### `GET /health`

The one route outside `/v1`, for the proxy and the deployment.
`200 {"status": "ok", "api_version": 1, "version": "2.3.0"}`.

#### `GET /v1/auth/start`

Opened in the system browser, not called by the desktop.

| Query | |
|---|---|
| `provider` | `github` |
| `port` | The desktop's loopback port, 1024 to 65535 |
| `state` | The desktop's state, 1 to 200 characters of `[A-Za-z0-9-._~]` |
| `challenge` | `base64url(sha256(verifier))`, 43 characters |
| `machine` | The machine's name, kept with the sign-in in progress; 1 to 100 characters, none of them a control character |

Each field once. `302` to the provider's consent page, with the server's own
`state` and `code_challenge` (S256), its `client_id`, its redirect URI and the
scopes above.

Since a browser asks, what is refused is answered with a **page** and the
error's status, not with JSON: `400` for a query the server would not follow,
`503` for a provider it has no client for, `429` (with `Retry-After`) past ten
starts a minute from one address or ten thousand sign-ins in progress in all,
`500` for its own fault. Nothing is stored, and the desktop's listener gives up
by itself after its ten minutes.

The pages (`crates/hangar-cloud/src/pages.rs`) are in English or French by
`Accept-Language`, self-contained (no script, image, font or link), carry
nothing of the request, and are sent with a policy that allows their one style
sheet by its hash and nothing else (`default-src 'none'; style-src
'sha256-…'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`).

#### `GET /v1/auth/callback/{provider}`

Where the provider sends the browser back: the redirect URI registered with
GitHub. Not called by the desktop. `302` to
`http://127.0.0.1:<port>/callback` with

- `?code=<link code>&state=<desktop state>` when the person is signed in;
- `?error=access_denied&state=<desktop state>` when they said no at the
  provider, `?error=provider_error&state=…` for any other failure there: an
  error the provider sent back, no code, a code it refuses, a profile it does
  not give or that has no id.

The server reads three things of the query, `state`, `code` and `error`, and
nothing else. A callback whose own state the server does not know (missing,
expired, replayed, forged, or started with another provider) is answered
with a `400` page and redirects nowhere: there is no desktop to send it to.
The server's own failure after that is a `500` page.

The desktop's listener checks `state` against its own. When it matches, it
answers the browser with a page saying the window can be closed and stops
listening; when it does not, it answers `400` and keeps waiting, for ten
minutes in all.

#### `POST /v1/auth/exchange`

```json
{ "code": "…", "verifier": "…", "machine": { "id": "…", "name": "Atelier", "os": "windows" } }
```

`200`:

```json
{
  "token": "hgr_…",
  "account": { "id": "…", "display_name": "Ada", "email": "ada@example.org",
               "avatar_url": "https://…", "providers": ["github"] },
  "device": { "id": "…", "name": "Atelier", "os": "windows",
              "created_at": 1794000000000, "last_seen_at": 1794000000000 }
}
```

`email` and `avatar_url` are left out when the provider gave none. The code is
consumed by the first exchange that presents it, whether or not the verifier
matches. `400 invalid_code` otherwise; `400 bad_request` for a machine id, name
or `os` that is empty, over 100 characters or holds a control character (the
code is then not consumed).

### With a device token

`Authorization: Bearer <device token>` on every request; `401 unauthorized`
without a valid one. A request moves the device's `last_seen_at`, at most once
a minute: a desktop that syncs often does not make the server write each time.

#### `GET /v1/me`

`200 {"account": {…}, "device": {…}}`: the account, and the device asking. What
the desktop calls at launch to learn whether it is still signed in.

#### `GET /v1/devices`

`200 {"devices": [{…}, …]}`: every device of the account, signed-out ones
included. A signed-out device carries `"revoked_at": <time>`; the field is
left out otherwise.

#### `PATCH /v1/devices/{id}`

`{"name": "Portable"}`, 1 to 100 characters. `200` with the device. Any device
of the account may rename any other. `404 not_found` for an id that is not the
account's.

#### `DELETE /v1/devices/{id}`

Signs the device out: its token stops working at once, its rows stay. With
`?forget=true` the device and its rows are deleted instead. `204`, with no
body. On oneself this is signing out, and the desktop then removes its token
from the keychain. Signing out a device that already is changes nothing (`204`);
`404 not_found` for an id that is not the account's.

#### `PUT /v1/stats`

```json
{ "rows": [ { "day": "2026-11-16", "model": "claude-opus-5-5", "project": "hangar",
              "input_tokens": 1200, "output_tokens": 3400,
              "cache_read_tokens": 560000, "cache_write_tokens": 78000,
              "cost_usd_micros": 1234567, "active_seconds": 5400, "sessions": 3 } ] }
```

The rows are those of the device whose token signs the request; a device
cannot write another's. Each row **replaces** the device's row of the same
(`day`, `model`, `project`): the values are totals, not increments, so sending
the same rows twice changes nothing, and a day still in progress is simply
sent again with its new totals. A key sent twice in one call keeps its last
row. Rows that are not sent are left as they are.

The desktop never lowers a day it already pushed: Claude Code deletes old
transcripts, so a total computed again later can come out smaller than the one
the server has, and the sync then leaves that day alone.

`200 {"upserted": <number of distinct keys>}`. The call is all or nothing:

- `413 too_large` over 5000 rows (the desktop sends more in several calls);
- `400 bad_request`, naming the first row at fault, when a `day` is not a
  calendar day, a `model` is empty or over 100 characters, a `project` is over
  100 characters (it may be empty) or holds `/` or `\`, either holds a control
  character, or a counter is over 2^63 − 1.

`cost_usd_micros` is the API-equivalent cost in millionths of a US dollar.
Lengths are counted in characters, not bytes.

#### `GET /v1/stats`

`?from=YYYY-MM-DD&to=YYYY-MM-DD`, both inclusive, either or both left out.

```json
{ "devices": [ { "id": "d1", "name": "Atelier", "os": "windows", "created_at": 1, "last_seen_at": 2 } ],
  "rows": [ { "device_id": "d1", "day": "2026-11-16", "model": "…", "project": "hangar", "…": 0 } ] }
```

Every row of the account in the range, from all its devices, the asking one
included, with the devices those rows name (signed-out ones too). A row is a
`PUT` row with `device_id` beside its fields. `400 bad_request` for a `from` or
a `to` that is not a day.

#### `DELETE /v1/account`

Deletes everything (see "Deleting"). `204`. Every token of the account,
this one included, is `401` from then on.

### Limits

| | |
|---|---|
| Rows per `PUT /v1/stats` | 5000 |
| `project` | 100 characters, may be empty, no `/` or `\` |
| `model` | 1 to 100 characters |
| Device name, machine id, `os` | 1 to 100 characters, no control character |
| `state` | 1 to 200 characters of `[A-Za-z0-9-._~]` |
| Verifier / challenge | 43 to 128 / 43 characters |
| Loopback port | 1024 to 65535 |
| Counters | 0 to 2^63 − 1 |
| Request body | 4 MiB |
| A sign-in, from its start to the provider's callback | 10 minutes |
| Link code, from the callback to the exchange | 5 minutes |

And per caller, a minute (`crates/hangar-cloud/src/limit.rs`): 300 requests
under `/v1` from one address, of which 10 exchanges and 10 starts; 120 requests
with one device's token. `429 rate_limited` past them, with `Retry-After`.

They are constants of `hangar-cloud-api`, which the desktop applies before it
sends: a folder name over 100 characters is cut to 100.

### Versioning

`API_VERSION` is the `1` of `/v1`. Inside a version the protocol only grows:
new routes, new optional fields, new error codes. Both sides ignore fields
they do not know and neither may require a field the other did not always
send. Anything else (a field removed or renamed, a meaning changed) is a `/v2`
beside `/v1`, which the server keeps answering for as long as desktops that
speak it are in use. `GET /health` says which version a server speaks.

## Running the server

Configuration is the environment, read once at start
(`crates/hangar-cloud/src/config.rs`). A variable that is set and wrong stops
the server with a sentence naming it.

| Variable | Default | |
|---|---|---|
| `HANGAR_CLOUD_BIND` | `0.0.0.0:8080` | The address and port to listen on. |
| `HANGAR_CLOUD_DB` | `/data/hangar-cloud.sqlite` | The SQLite file. |
| `HANGAR_CLOUD_BASE_URL` | `https://account.hangarai.dev` | The server's public origin (scheme and host, no path): the OAuth redirect URIs are built from it. |
| `HANGAR_CLOUD_TRUSTED_PROXIES` | `1` | How many reverse proxies are in front of the server, 0 to 8. `1` is Cloudflare and nothing else; `2` when another proxy sits between Cloudflare and the container; `0` when the server is reached directly (development). See "Cloudflare" below. |
| `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` | none | The GitHub OAuth app. Both or neither. |

Without the two GitHub variables, sign-in is off: `/v1/auth/start` answers
`503`. The redirect URI to register with GitHub is
`<HANGAR_CLOUD_BASE_URL>/v1/auth/callback/github`; the server prints it at
start, with the client's id and never its secret.

GitHub's address is not configuration: the server calls `github.com` and
`api.github.com` over HTTPS, follows no redirect from them, and no variable
points it elsewhere. Only the tests stand something in for them
(`crates/hangar-cloud/src/tests/fake.rs`).

While developing, on a port of one's own:

```sh
HANGAR_CLOUD_BIND=127.0.0.1:21120 HANGAR_CLOUD_DB=./cloud.sqlite \
HANGAR_CLOUD_BASE_URL=http://127.0.0.1:21120 HANGAR_CLOUD_TRUSTED_PROXIES=0 \
cargo run -p hangar-cloud
HANGAR_CLOUD_URL=http://127.0.0.1:21120 cargo run -p hangar-app
```

To sign in against it, register a second GitHub OAuth app with
`http://127.0.0.1:21120/v1/auth/callback/github` as its callback (GitHub
accepts a loopback address over plain HTTP), and give the server its id and
secret: never the production ones.

## Deploy

One container, one volume, behind Cloudflare at
`https://account.hangarai.dev`, run by `crates/hangar-cloud/compose.yaml`.
The person deploys; CI only builds, checks and publishes the image (step 3).

### 1. The GitHub OAuth app

An **OAuth App**, not a GitHub App (`read:user user:email` are OAuth scopes).
On GitHub: Settings, Developer settings, OAuth Apps, New OAuth App (under the
`hangar-ai` organization's settings rather than a personal account's, so that
the consent page names the organization).

| Field | Value |
|---|---|
| Application name | `HangarAI` |
| Homepage URL | `https://hangarai.dev` |
| Authorization callback URL | `https://account.hangarai.dev/v1/auth/callback/github` |
| Enable Device Flow | off |

Then "Generate a new client secret". The client ID is `GITHUB_CLIENT_ID`, the
secret `GITHUB_CLIENT_SECRET`; GitHub shows the secret once.

The callback URL must match to the character: `https`, no trailing slash, no
port. It is `HANGAR_CLOUD_BASE_URL` followed by the path, and the server
prints it at start.

### 2. The settings

On the host, in a folder of its own (`/opt/hangar-cloud`, say), the Compose
file and the settings, which hold the client's secret: readable by their
owner only, never in the shell's history or on a command line.

```sh
sudo install -d -m 700 /opt/hangar-cloud && cd /opt/hangar-cloud
base=https://raw.githubusercontent.com/hangar-ai/hangar/main/crates/hangar-cloud
sudo curl -fsSLO "$base/compose.yaml"
sudo curl -fsSL "$base/hangar-cloud.env.example" -o hangar-cloud.env
sudo chmod 600 hangar-cloud.env
sudoedit hangar-cloud.env
```

`hangar-cloud.env` (`crates/hangar-cloud/hangar-cloud.env.example`):

```
GITHUB_CLIENT_ID=…
GITHUB_CLIENT_SECRET=…
# Only when they differ from the defaults (see "Running the server"):
# HANGAR_CLOUD_TRUSTED_PROXIES=1
# HANGAR_CLOUD_BASE_URL=https://account.hangarai.dev
```

No quotes around the values: Compose keeps them as written. A value left
empty is no value: without both GitHub values, sign-in is off.

### 3. The image

CI builds the image whenever the server changes, checks it as a host runs
it (it starts with no secret, answers `/health`, refuses `/v1` without a
token, reports healthy, writes its database as its own user on a read-only
root, stops within seconds), and publishes it from `main` and from release
tags (`.github/workflows/cloud-image.yml`):

| Tag | What |
|---|---|
| `ghcr.io/hangar-ai/hangar-cloud:latest` | the last commit of `main` that changed the server |
| `ghcr.io/hangar-ai/hangar-cloud:sha-<commit>` | that commit, to pin or to go back to |
| `ghcr.io/hangar-ai/hangar-cloud:<version>`, `:<major>.<minor>` | a release (`v2.4.0` gives `2.4.0` and `2.4`) |

`compose.yaml` runs `latest`; `HANGAR_CLOUD_TAG=sha-1a2b3c4 docker compose up -d`
(or the same line in a `.env` beside it) pins another.

Once, after the first publish: GitHub publishes a new package as private.
Either make it public (the organization's Packages, `hangar-cloud`, Package
settings, Change visibility: the image holds the open-source server and no
secret), or sign the host in to the registry with a token that has
`read:packages` (`docker login ghcr.io`).

The published image is for x86-64. On an arm64 host, or to run a change not
yet on `main`, build it from a clone of the repository instead, with BuildKit
(Docker 23 or later), where `docker compose up -d --build` builds it beside
the Compose file:

```sh
git clone https://github.com/hangar-ai/hangar && cd hangar/crates/hangar-cloud
cp hangar-cloud.env.example hangar-cloud.env && chmod 600 hangar-cloud.env   # then fill it in
docker compose up -d --build
```

(`crates/hangar-cloud/*.env` is ignored by git and left out of the image's
build context.)

### 4. Run it

Behind a reverse proxy on the host (Caddy, Traefik, nginx, Coolify,
Dokploy), which reaches the server at `127.0.0.1:8080`
(`HANGAR_CLOUD_PORT` in a `.env` beside the Compose file picks another port):

```sh
docker compose up -d
docker compose ps        # hangar-cloud … (healthy)
docker logs hangar-cloud
```

Or through a Cloudflare Tunnel, with no reverse proxy and no port open to the
internet at all: in Cloudflare, Zero Trust, Networks, Tunnels, create a tunnel
(cloudflared), add the public hostname `account.hangarai.dev` with the
service `HTTP` `hangar-cloud:8080`, and put the tunnel's token in
`cloudflared.env` (`crates/hangar-cloud/cloudflared.env.example`):

```sh
sudo curl -fsSL "$base/cloudflared.env.example" -o cloudflared.env
sudo chmod 600 cloudflared.env && sudoedit cloudflared.env    # TUNNEL_TOKEN=…
docker compose --profile tunnel up -d
```

The tunnel starts once the server reports healthy. The tunnel is the case
`HANGAR_CLOUD_TRUSTED_PROXIES=1` (the default) is for; behind Cloudflare and
a reverse proxy, set it to `2` (step 5).

The server runs as a user of its own (uid 10001) on a read-only root, with no
capability, listens on 8080 and keeps its one file in the volume
`hangar-cloud-data`: that volume is the whole state of the service. The log's
first lines say what it runs with, and GitHub's client id and redirect URI,
or "not configured". They never hold a secret, and the server
logs no request afterwards: only its own failures, as a step and a status.
`hangar-cloud health` (the image's health check) asks its own `/health`.

The port is published to the host only (`127.0.0.1`), for a tunnel or a
reverse proxy on the same host. Never publish it to the internet: see below.

Without Compose, the same container is:

```sh
docker run -d --name hangar-cloud --restart unless-stopped \
  -p 127.0.0.1:8080:8080 -v hangar-cloud-data:/data \
  --env-file hangar-cloud.env \
  --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges \
  ghcr.io/hangar-ai/hangar-cloud:latest
```

### 5. Cloudflare

DNS: a record for `account.hangarai.dev` (the name `account` in the
`hangarai.dev` zone), pointing at the host or at the tunnel, **proxied** (orange
cloud), like the site's. TLS is Cloudflare's: SSL/TLS mode "Full (strict)" when the
origin has a certificate, and "Always Use HTTPS" on. The server sends
`Strict-Transport-Security` itself.

How Cloudflare reaches the container decides `HANGAR_CLOUD_TRUSTED_PROXIES`.
The rate limits are per caller, and behind a proxy the caller's address is in
`X-Forwarded-For`, to which each proxy appends the address it received from.
The setting is the number of entries that header holds when a request that
sent none reaches the container:

| In front of the container | Setting |
|---|---|
| Cloudflare, through a Cloudflare Tunnel (`cloudflared`, the Compose file's `tunnel` profile) | `1` (the default) |
| Cloudflare, then a reverse proxy on the host (Caddy, Traefik, nginx, a PaaS's router) that appends to `X-Forwarded-For` | `2` |
| Nothing (development) | `0` |

Too high, and the server believes an address the caller wrote, who then
escapes the limits by changing it. Too low, and everybody is counted as the
proxy and shares one limit: sign-ins start failing with `429` under ordinary
use. Some reverse proxies replace the header instead of appending to it unless
told to trust Cloudflare's addresses; rather than guess, put a header-echoing
container (`traefik/whoami`) behind the same route once, call it through the
public name, and count the entries of the `X-Forwarded-For` it shows.

The origin must be reachable **only** through Cloudflare, or the header is
whatever a direct caller says: a tunnel, or a firewall that lets in
Cloudflare's address ranges and nothing else, with the container's port
published on the loopback only.

For this hostname, in Cloudflare:

- no challenge: Bot Fight Mode, "Under Attack" and managed challenges answer
  with a page that a browser solves and the desktop cannot. The desktop calls
  `/v1/…` with a plain HTTP client; a challenge there looks like the server
  being down. Skip them for `account.hangarai.dev`;
- no cache rule: every answer says `Cache-Control: no-store`, and some carry a
  token. Do not add "Cache Everything";
- nothing injected into HTML (Rocket Loader, email obfuscation, the Web
  Analytics snippet): the sign-in pages' policy refuses every script, so it
  would only fill the browser's console with refusals.

### 6. Check it

```sh
curl -fsS https://account.hangarai.dev/health
# {"status":"ok","api_version":1,"version":"2.3.0"}
curl -sI https://account.hangarai.dev/v1/me | head -1
# HTTP/2 401
```

`GET /health` is outside `/v1` and outside the rate limits: point an uptime
monitor (Cloudflare's health checks, or any other) at it. The image's own health check (`hangar-cloud health`) asks the same route
from inside the container: `docker compose ps` shows it.

Then sign in for real, from a Hangar with no `HANGAR_CLOUD_URL` set. What a
mistake looks like:

| What you see | What it is |
|---|---|
| GitHub: "The redirect_uri is not associated with this application" | The callback URL of step 1, or `HANGAR_CLOUD_BASE_URL`. |
| The server's page "This sign-in method is not available" (`503`) | The container did not get the two GitHub variables: `docker logs hangar-cloud` says "not configured". |
| Hangar says the provider failed (`provider_error`) | `docker logs hangar-cloud` names the step: "failed at the code's exchange: status 401" or "the provider refused the code" is a wrong client secret. |
| `429` after a few sign-ins | `HANGAR_CLOUD_TRUSTED_PROXIES` is too low (step 5). |
| The desktop cannot reach the server at all, the browser can | A Cloudflare challenge on `/v1` (step 5). |

### Backups

The database is one SQLite file in WAL mode: a plain copy of it while the
server runs can be torn. Take a snapshot with SQLite's own backup, which is
consistent and does not stop the server. The image has no `sqlite3`; a
throwaway container beside it does:

```sh
#!/bin/sh
# /usr/local/bin/hangar-cloud-backup, run daily by cron or a systemd timer.
set -eu
dir=/var/backups/hangar-cloud
install -d -m 700 "$dir"
docker run --rm -v hangar-cloud-data:/data -v "$dir":/backup alpine:3 sh -c '
  apk add --no-cache sqlite >/dev/null &&
  sqlite3 /data/hangar-cloud.sqlite ".backup /backup/hangar-cloud-$(date +%F).sqlite" &&
  chmod 600 /backup/hangar-cloud-*.sqlite'
find "$dir" -name 'hangar-cloud-*.sqlite' -mtime +14 -delete
```

- **Fourteen days, no more.** The privacy page promises that what a person
  deletes is gone from the backups within 14 days. Keep the `find` line, and
  apply the same limit to any copy sent off the host.
- A backup holds names and email addresses, and the hashes of tokens (not
  tokens). Keep it readable by root only, and encrypt it before it leaves the
  host.
- Test a restore once before it is needed.

To restore: stop the container, put the snapshot in the volume as
`hangar-cloud.sqlite`, remove any `hangar-cloud.sqlite-wal` and `-shm` beside
it, give it to uid 10001, start the container.

```sh
docker stop hangar-cloud
docker run --rm -v hangar-cloud-data:/data -v /var/backups/hangar-cloud:/backup alpine:3 sh -c '
  rm -f /data/hangar-cloud.sqlite-wal /data/hangar-cloud.sqlite-shm &&
  cp /backup/hangar-cloud-YYYY-MM-DD.sqlite /data/hangar-cloud.sqlite &&
  chown 10001:10001 /data/hangar-cloud.sqlite'
docker start hangar-cloud
```

A restore brings back the accounts that were deleted since the snapshot. Their
tokens were destroyed on the machines, so nobody is signed in to them again by
it, but the data is back: delete those accounts again, from the list of what
was deleted since if one is kept, or do not restore further back than needed.

### Updating

```sh
/usr/local/bin/hangar-cloud-backup
cd /opt/hangar-cloud && docker compose pull && docker compose up -d
```

(From a clone: `git pull && docker compose up -d --build`.)

The schema's migrations run at start. A database that a newer server has
migrated is refused by an older one, which says so and stops: going back a
version means restoring the snapshot taken before the update. Machines stay
signed in across an update, their tokens being in the database.

To change the client secret: generate the new one on GitHub, put it in
`hangar-cloud.env`, and run `docker compose up -d` (it recreates the
container when its settings changed; `docker restart` does not read the file
again). Nobody is signed out by it.

## Status

Built: this document, the protocol's types, the server (storage, device
tokens, machines, statistics, account deletion, the sign-in with GitHub and
its pages), the desktop's calls, sign-in and sync, the Hangar
account section of the settings, the "This PC / All machines" switch of the
statistics, the site's privacy page. Not done yet: the first deployment (see "Deploy").
