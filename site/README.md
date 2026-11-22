# site

The HangarAI landing page, served at https://hangarai.dev. Plain HTML, CSS
and JavaScript, no framework, no build step for the page itself, and nothing
loaded from another origin.

```
index.html            the page
privacy.html          privacy and cookies: what each part stores, the GDPR's controller, legal bases,
                      recipients and transfers, rights, and every cookie (see The legal pages)
legal/                the legal notice, the app terms, the affiliate terms and their legal.css
terms/                HangarAI Cloud's terms
styles.css            every colour is a token; Graphite unless main.js swaps them
main.js               themes, section bar, download per OS, the film, folds
assets/motion.js      the scroll-driven motion (see Motion below)
assets/mobile.css     the Hangar Mobile section: the phones drawn around their screens
vendor/               GSAP 3.15.0 and its ScrollTrigger, unchanged, and their licence
img/                  crops of captures of the app, AVIF + WebP, cut as img/crops.json says
video/                the 3.0 film (EN, FR; 1080p and 720p), its covers and the night shift's still
fonts/                Inter (latin, variable, with optical sizes) and its OFL licence
og.png                the 1200×630 share image for X, Open Graph and the rest
favicon.ico           16/32/48 px, for whatever still asks for /favicon.ico
apple-touch-icon.png  180 px
logo.svg              copied from ../assets/logo.svg
```

## Preview

```bash
npx serve site           # or: python -m http.server -d site 8080
```

The page used to carry a browser demo, the React app built for the web with a
scripted backend. The app is native now, so there is no web build of it to show:
the demo went with the React sources (it is at tag `v1.12.1`, under `demo/`).

## The page

It sells what you get, then shows how, in this order:

1. **Hero**: the outcome, in three headlines that take turns, each with its
   sentence (a week of work in a day; your AI dev team, you call the shots;
   it keeps shipping while you sleep), *Start the 14-day trial* (the
   installer for the visitor's system, marked in the row of systems under
   it) and *Watch the 40 s film*; three figures, each true and sourced (40
   panes per workspace, ten deliveries landed in 65 s, overnight); the
   workspace in its « Chat + team » tiles. The first headline is the page's `<h1>`, and the one a visitor
   without script or with reduced motion sees; the other two are drawn over
   it in the same grid cell and are `aria-hidden` (no live region: nothing
   is read again and again). The cell is as tall as the tallest headline in
   the language on screen, at every width, so a change moves nothing.
2. **The problem**: five pains of running agents by hand, one line each.
3. **How it works**: three steps (the objective, the orchestrator hands out the
   work, finished work lands together).
4. **The film** (below).
5. **The 3.0 pillars**, one block each, a sentence of outcome and two to four
   facts beside a real picture: a team, not a chatbot (the panes); lands fast
   and safe (the landing measured, in HTML); works while you sleep (the night
   shift's sheet, a still of the film); cloud power (Cloud Builds' price
   sheet); you stay in control (two Hangar Mobile screens).
6. **Also in the box**, the theme swatches, and *Native, in Rust* as one
   measured line; pricing (the numbers of `pricing/`); downloads; questions.

The sections before 3.0's (`#chat`, `#orchestrator`, `#mobile`, `#kits`,
`#experiments`, `#performance`) are anchors on the sections that took their
place (`.alias`), since the other pages' menus still name them.

Hangar runs Claude Code and Codex: the page names both, Codex always marked
beta, and no other agent.

Every claim on it comes from the code, `CHANGELOG.md` or the English strings,
true on main the day it is written: what 3.0 does not ship is not on it (hosted
nodes and the GPU machines of Cloud Builds are on it, since main has both).
When a feature changes, check the copy against
`crates/hangar-i18n/dict/en.json` (the English strings say what the app does in
plain words; the French ones give the app's own French words: *fusionner*,
*Besoin de toi*, *Mode nuit*, *lancement*) and against the file the claim
names: `crates/hangar-app/src/state/types.rs` for the defaults,
`crates/hangar-core/src/delivery.rs` for the delivery and its speed modes,
`crates/hangar-app/src/settings/experiments.rs` for the experiments,
`crates/hangar-app/src/cloud/service.rs` for what HangarAI Cloud offers.

The figures are measures only. The 65 s is
`delivery::tests::together::measure_ten_ready_deliveries_one_at_a_time_and_together`
(`docs/native/DELIVERIES.md`, "Ten ready at once": 64.5 s, rounded as the
changelog does; the page cites the changelog, which is public); the memory
line is `docs/native/BENCHMARK.md`'s, 1.12.1 against 2.0. Add a figure only
with the measure it comes from.

On a phone (below 600 px) the page keeps every point and drops detail: the
words wrapped in `.wide-only` are left out there, so read them with the rest
when the copy changes. The short facts (under the first pillar and "Also in
the box") are `<details class="fold">`, written open; `main.js` folds them to
their titles on a phone, and above 600 px they simply stay open.

The landing card (`#race`) is HTML, not a capture: the measure's two times and
their bars, written at their end.

### The film

`video/` holds the 40 s film of `marketing/trailer`, served from the site. Its
cover is the film's first title in the page's language (a still, AVIF and
WebP); nothing of the film loads before a click (`preload="none"`), which plays
the one in the page's language, with its sound, and gives the player's own
controls. The hero's *Watch the 40 s film* brings it to the middle of the
screen and starts it in the same click (so the browser lets the sound play).
A `media` source picks 1080p from 800 px up, 720p below. Without script, the
players show with their controls.

Made from the trailer's renders (`~/dev/hangar-trailer/final`, see
`marketing/trailer/README.md`):

```bash
T=~/dev/hangar-trailer/final
# 1080p: the renders themselves (H.264 at CRF 14, about 2.9 Mb/s: the film is
# flat colour and text), the index moved to the front for streaming
ffmpeg -i $T/hangarai-3.0-trailer-en.mp4 -map 0:v:0 -map 0:a:0 -c copy -map_metadata -1 \
  -movflags +faststart video/hangarai-3.0-en-1080p.mp4          # and -fr
# 720p, about 1.1 Mb/s (English from the ProRes master)
ffmpeg -i $T/hangarai-3.0-trailer-en-master.mov -map 0:v:0 -map 0:a:0 \
  -vf "scale=1280:720:flags=lanczos,format=yuv420p" -c:v libx264 -preset slower -crf 20 \
  -maxrate 3500k -bufsize 7000k -profile:v high -level 4.1 -g 120 \
  -c:a aac -b:a 160k -ac 2 -ar 48000 -map_metadata -1 -movflags +faststart \
  video/hangarai-3.0-en-720p.mp4                                 # and -fr, from its .mp4
# covers: the first title (1.6 s), 960 and 1920 wide; the night shift's sheet
# (21.6 s, cropped 1320 × 728 at 300, 290), 660 and 1320 wide; each as WebP
# (-c:v libwebp -quality 82) and AVIF (-c:v libaom-av1 -still-picture 1 -crf 30)
```

Their `?v=` stamps are the first ten hex digits of each file's SHA-1, as the
crop script writes them, but by hand: `scripts/site-crops.mjs` does not know
`video/`. Stamp them again when a file changes.

## Motion

`assets/motion.js`, with GSAP and ScrollTrigger from `vendor/` (3.15.0, the
npm package's `dist/` files as they came; `vendor/GSAP-LICENSE.md` holds the
licence and their checksums). About 46 KB gzipped together; nothing comes from
another origin. To update GSAP, copy the two files from the new package, change
the `?v=3.15.0` of their `<script>` tags, and the version and checksums in the
licence file (the crop script stamps `assets/motion.js`, not `vendor/`).

What moves, top to bottom:

- **Hero.** At load, the headline rises word by word and the rest of the copy
  after it (timed, once). Then the three headlines take turns, 4.5 s each
  (`TUNE.headlineFor`): the one on screen and its sentence fade out, a little
  blurred and a few pixels up, as the next come in the same way, word by
  word; the pill under them fills meanwhile, and a click on a pill goes to its
  headline. They wait under the pointer, while a pill has the keyboard's
  focus, while the tab is hidden and while the hero is off screen; a change
  of language keeps the headline on screen. As you scroll, the window leans back in perspective
  and lies flat by the time its top nears the bar, the copy fades back, and the
  accent glow drifts down, slower than the page.
- **A team, not a chatbot.** The panes window leans back and lies flat as it
  comes in.
- **Lands fast, and safe.** As the card comes in, 65 counts up and the two bars
  grow from nothing: together quickly, one after the other slowly.
- **You stay in control.** The two phones come in one after the other, from
  depth, fanned out.
- **Everywhere.** Section titles rise line by line out of a mask; cards, steps,
  facts and short texts fade up as they enter; links within the page glide to
  their section (any wheel, touch or key stops the glide).

How: `gsap.matchMedia()` with three sets. `full` (960 px and wider) does all of
the above; `light` (narrower) drops the tilts and the phones' depth for plain
rises and fades; `reduce` is next.

**Reduced motion** (`prefers-reduced-motion: reduce`): nothing travels, scales
or counts. Titles, cards and texts only fade in, and the landing card shows
its final state. The headlines do not turn by themselves: the first stays,
and a pill changes it with a slow fade. The inline script in `<head>` does not even set
`motion` then.

Without script everything is visible as written: the only CSS that hides
anything (the hero's copy, before its entrance) applies under `html.motion`,
which the inline script sets before the first paint and takes back after 3 s if
`motion.js` never set `motion-ready`.

Tuning: the numbers live in `TUNE` at the top of `motion.js` (scrub lag, the
hero's tilt, scale, lift and where it lies flat, the glow's drift, the
windows' tilt, the reveals' rise, the line stagger, the counting time). Rules
to keep: move `transform` and `opacity` only (the exception is the landing's
figure while it counts, held at its final width so nothing shifts); give back
the final state on the way out of a set (each returns its cleanups). Check with
the page served locally and headless Chrome at 1440 and 390 px, with and
without reduced motion, and a layout-shift observer while scrolling: it should
read 0.

## Design

The page wears the app's own chrome tokens. `styles.css` holds Graphite, the
default theme, exactly as `crates/hangar-theme` has it, and GitHub Light for
a visitor whose system is light (`data-theme="auto"` on `<html>`, a
`prefers-color-scheme` block). `main.js` holds the same tokens for eight
themes, two of them light, and the swatches under "Also in the box" repaint
the page; picking one sets `data-theme` to its id and `data-scheme` to light
or dark.

The hero has a capture in each scheme (`hero`, `hero-light`): the page shows
the one that matches the theme on screen, and the other, `loading="lazy"`
under `display: none`, is never fetched.

Graphite keeps colour for meaning, and so does the page: green is an agent at
work, amber one waiting on you, cyan your turn, blue the focus and the thing
to click. The one decoration is a soft pool of the theme's accent behind the
hero. Radii follow the app: 4 to 6 px for controls, 8 for cards, 10 for
floating panels and windows. Type is Inter, as in the app, with its optical
sizes doing the display cut of the headlines; monospace is only for what a
terminal would print.

Every text token clears 4.5:1 on every surface it sits on, in all eight themes.

To add a theme to the swatches, take its tokens from the app rather than
picking them by eye. The theme crate prints them:

```bash
cargo run -q -p hangar-theme --example kit-tokens -- kanagawa
```

then map the `--*` names to the keys `TOKENS` lists in `main.js`.

## Captures

Every picture in `img/` is the real app, HangarAI 3.0 in its tiles, on a demo
written for the site: made-up names and work, and no path, account or project of the
person taking them. One command makes them all again:

```bash
PYTHON=<a Python whose Pillow writes AVIF> scripts/site-shots/shoot [captures folder]
```

It needs the repository's packages (`pnpm install`), `google-chrome` (or
`$CHROME`) and the build machine of `scripts/remote` (nothing is compiled on
the PC). In order:

1. **The windows.** hangar-app's ignored test `marketing::pictures`
   (`crates/hangar-app/src/marketing.rs`) draws the main window headless on the
   build machine, the way the app draws it: Hangar's own title bar, the themes,
   Inter, and JetBrains Mono for the terminals (embedded in the test, so the
   machine's fonts make no difference), at scale 2, on the software renderer.
   Its PNGs are fetched back (`rbuild ssh`). Each window is drawn twice: once
   for the grid and the terminals to measure themselves, then with each
   terminal at that size and filled.
2. **Hangar Mobile.** `scripts/site-shots/mobile.mjs` serves `mobile/` in its
   mock mode with Vite (on `HANGAR_PORT_BASE`) and has a headless Chrome draw
   four screens at 390 × 844 and scale 2, with an iPhone's safe areas. The
   mock's PC and one of its workspaces bear the names of the person who wrote
   it: they are renamed as the files are served (`STUDIO-PC`, `hangar-ai`),
   never in `mobile/`.
3. **`img/`**, cut by `scripts/site-crops.mjs` as `img/crops.json` says
   (below), which also stamps the page.
4. **`og.png`**: `scripts/site-shots/og.html` (the brand, the headline, one
   line on the orchestrator, the hero bleeding off the bottom) rendered at
   1200 × 630; and the README's `docs/images/chat-view.png`, the hero
   (« Chat + team ») at 1440 × 900.

The demo (`crates/hangar-app/src/marketing/recipes.rs`) is a **Recipes**
workspace. Iris, the orchestrator (Claude Code, Opus), planned the app as
tasks on the board and started workers; Ada's data model landed and her
sample recipes passed their check and review, waiting to be merged; Cleo
(Codex) delivered the recipe page, which Hangar is checking on a Cloud Builds
machine; Ben (Claude Code) sorts the list, Dan (Codex) writes the search, Eva
(Claude Code) waits for a permission to add a dependency; sharing a recipe by
link waits on a question only the person can answer. The app's own packages
hold all of it as they would: the board in its cache, the panes as the
bridge reports them, Iris's conversation as the desk reads it, the deliveries'
stages, Needs you, the license and the Cloud space. What the agents printed
is `marketing/screens.rs`, written for the size each terminal measures. No
process, file or network is involved.

The pictures show what 3.0 ships, and nothing it does not: every service
`cloud::service` offers (`shipped`, hosted nodes included) is shown live, and
one it does not offer, if there were one, would never be shown, not even as to
come.
So Cleo's delivery is checked on a Cloud Builds machine ("Checking in the
cloud"), the status bar carries the cloud chip, and the Cloud space holds
that machine, the production build setting up, a hosted node keeping Garden
planner going with the computer off (the plan's third machine, so that the
job behind it, another workspace's, waits in the queue), the recipe page
shared by link, and a history of the images Cleo had drawn and of dictation
(`cloud::demo`). A trial rents no machine, so the
pictures on a trial (`trial`, `upgrade`, `plans`) check the delivery on the
computer and run nothing in the cloud. No page may tag a feature "Later" or
"Coming": what is not built is left out.

The captures, all of the main window:

| Capture | What | Pixels |
|---|---|---|
| `chat`, `light` | the hero: Recipes in « Chat + team », the conversation with Iris pinned on the left, and the tiles that arrived by themselves on the right: Needs you, Agents, Cloud (its runs and what they cost), Machines; Graphite and GitHub Light | 2880 × 1800 |
| `chatm`, `chatml` | the same alone in a 760 × 900 window, its tiles stacked, for the phone's hero | 1520 × 1800 |
| `panes` | « Terminals »: Iris, Ben, Dan and Eva at work, Eva's permission in amber; the navigator closed and the window 1520 wide, for the headers' room | 3040 × 1800 |
| `board` | the board, every stage, zoomed over « Chat + team » from the bar | 2880 × 1800 |
| `cloud`, `cloud-light` | the Cloud space at work: the delivery's check on a Cloud Builds machine, a build setting up, a hosted node, a job queued behind them, a share link, the history of images and dictation (no page shows it now) | 2880 × 1800 |
| `cloud-builds`, `cloud-builds-light` | Cloud Builds' price sheet over « Chat + team »: the machine picker (the catalog's cards, an RTX 4090 picked for Recipes' photo tagger), the rate on the best machine free now, the files that go and the estimate (the pricing page's Pro or Max, the home's cloud pillar) | 2880 × 1800 |
| `license`, `license-light` | Settings › License: HangarAI Pro, Upgrade to Max, this computer, the credits (the pricing page's credits) | 2880 × 1800 |
| `upgrade`, `upgrade-light` | the plans sheet a trial's badge opens: Pro and Max as the server sells them (the pricing page's offers, cut as `plans`) | 2880 × 1800 |
| `plans`, `plans-light` | the screen after a trial: the plans, the key field | 2880 × 1800 |
| `trial` | the hero on a trial: its badge in the title bar | 2880 × 1800 |
| `nodes` | Settings › Nodes, a node connected with Claude Code and Codex | 2880 × 1800 |

The 3D preview (`model`, `model-m`) is not among them: its viewport is drawn
by the GPU, which the test's software renderer has not, so those two are still
the 2.2 capture of a debug build (`HANGAR_DEV_CAPTURE_DIR`, `kill -USR1`).
`shoot` cuts only the images whose capture it has.

Before a capture goes public, check it frame by frame: no `/home/<you>` or
`C:\Users\…` path, no e-mail address, no other project, and no "bypass
permissions" banner.

`img/crops.json` says what is cut out of the captures, and
`scripts/site-crops.mjs` cuts it:

```bash
node scripts/site-crops.mjs --sources <folder of captures>          # every image
node scripts/site-crops.mjs --sources <folder> --only hero,board     # some of them
```

The pixels are Pillow's, through Python, and it must write AVIF (Pillow 11.2
or later from pip; a distribution's Pillow may lack it): point `PYTHON` at
one that does, such as a virtual environment's.

For each image, `crops.json` gives the capture it comes from, a rectangle in the
capture (`[x, y, width, height]`, in capture pixels), the widths to write, and
optionally a fade over an edge the crop cuts through. The first width sets the
`width` and `height` the page reserves, and an image is never drawn wider than
that, so it must be at least the width the layout gives the image. Every width
becomes `img/<name>-<width>.avif` (quality 62) and `.webp` (quality 82), and
widths an image no longer has are deleted. The script then writes into each
page `crops.json` lists under `pages` (`index.html`, `pricing/index.html`,
each with its way to `img/`) the `srcset`, `src`, `width` and `height` of
each `<source>` and `<img>` marked `data-crop="<name>"`, and the stamps of
what the page loads. It can also place numbered outlines over an image
(`marks`, `<span class="mark" data-of="…">`); the pages use none now.

The pricing page is in English and French: an image there carries its French
in `data-fr` beside an `aria-label` that `assets/lang.js` swaps, and its
caption in the page's `data-l` spans.
It warns when a width asks for more pixels than the capture has there, or a
rectangle runs off it.

Every URL it writes carries `?v=` and the start of the file's hash, and so do
`styles.css`, `main.js`, `assets/mobile.css` and the share image `og.png`. The
site sends images, styles and scripts with a four-hour browser cache, so
without it a visitor who came before an update would get the new page with the
old images under it. After changing `styles.css`, `main.js` or `og.png` by
hand, run the script again (`--html-only` is enough) so their stamps follow.

The full-window captures are not kept in the repository; `--sources` names the
folder that holds them. To swap them, put the new ones in a folder, set each
image's `src` and `rect`, and run the script. Nothing in the page is edited by
hand.

Below 600 px each `<picture>` switches, through a `media` source, to the `-m`
crop of the same image, drawn close to the size the app draws it so its text
reads on a phone: the narrow « Chat + team », one pane (Eva's permission). Everything
below the hero is `loading="lazy"`. Some crops (`tasks`, `board`, `model`,
`nodes`, `cloud`, two of the phones) are on no page since 3.0's home; the
crop script says so, and they stay cut for a page that wants them.

## The legal pages

Every page's footer has a *Legal* column with the same links, in this order:
*Legal notice* (`legal/notice.html`), *App terms* (`legal/terms-app.html`),
*Cloud terms* (`terms/`), *Affiliate terms* (`legal/affiliate-terms.html`),
*Privacy and cookies* (`privacy.html`), and the repository's *License*. The
share viewer (`s/`) has no footer of the site's: its own carries *Legal
notice* beside *Made with HangarAI*. The app links the first four from
Settings › About › Legal (`crates/hangar-app/src/settings/schema.rs`, the
`*_URL` constants): a page that moves must keep its address, or change there
too.

| Page | What it is | Its facts come from |
|---|---|---|
| `legal/notice.html` | The *mentions légales* the LCEN requires: publisher, director of publication, contact, hosts, the seller, intellectual property, credits | The publisher's own details (below), each host's and Polar's own legal pages |
| `legal/terms-app.html` | The app's terms: the license and its seats, the trial, the free license of 2.x installs, Pro and Max (prices, renewal, cancelling, plan changes), the right of withdrawal, acceptable use, ending, guarantees and liability, law and disputes. Polar's buyer terms call it the "Supplier Agreement" | `pricing/` and `CHANGELOG.md` for what the plans include; Polar's buyer terms for the sale |
| `terms/` | HangarAI Cloud: credits, refunds, each service's rules, takedowns | `crates/hangar-app/src/cloud/service.rs`, the server |
| `legal/affiliate-terms.html` | The referral program | `docs/native/referral.md` |
| `privacy.html` | What leaves the computer and what is kept, then the GDPR's parts: *Who is responsible* (`#controller`), *Why, and on what legal basis* (`#bases`), *Who receives it, and transfers outside the EU* (`#recipients`), *Your rights*, *Automated decisions*, *Security*, *Cookies and browser storage* (`#cookies`) | The server's code and `docs/native/*.md`; each provider's own privacy policy for its country and transfer mechanism |

All of them are in English and French in one file (`data-l`), as the rest of
the site. `legal/notice.html` and `legal/terms-app.html` were made from
`legal/affiliate-terms.html`: the same head, header and footer, another
`<main>`. A new legal page starts the same way, and gets a line in every
footer's *Legal* column.

**The publisher**, as the person gave it on 2026-11-22, published as is:
Simon Gay EI (*entrepreneur individuel*: the name always carries « EI », loi du
14 février 2022), trading as HangarAI, 11 rue de Douaumont, 42400
Saint-Chamond; SIREN 990 423 162, SIRET 990 423 162 00017, RCS Saint-Étienne,
NAF/APE 6201Z, created 20 August 2025, VAT FR30990423162; director of
publication Simon Gay; contact@hangarai.dev for everything (no phone
published). **If any of it changes** (an address, a company in place of the
EI, a phone number), change it in `legal/notice.html` (section 1, and the
address in section 12 of `legal/terms-app.html`) and in `privacy.html`
(`#controller` and the postal address under *Your rights*), and the date at
the top of each page.

**The hosts and providers**, checked on 2026-11-22 from their own pages: Vyme
SASU (MercuryCloud), 200 rue de la Croix Nivert, 75015 Paris, RCS Paris 989
030 978, +33 9 72 15 33 66 (mercurycloud.fr/legal/mentions-legales; the VPS's
address is in Vyme's AS199297); Cloudflare, Inc., 101 Townsend St, San
Francisco, CA 94107, +1 650 319 8930, certified to the EU-U.S. Data Privacy
Framework (its privacy policy); GitHub, Inc., 88 Colin P. Kelly Jr. Street,
San Francisco, CA 94107, certified to the DPF; Polar Software, Inc., 3500
South DuPont Highway, Dover, DE 19901, standard contractual clauses (its
privacy policy), buyer terms of 25 March 2026, quoted in the app terms'
section 6; Groq, fal, Vast.ai: contractual clauses, as their policies say;
Hetzner Online GmbH, in the EU. A new provider goes in `privacy.html`'s
providers table, in `#recipients` with its country and transfer mechanism,
and, if it hosts the site, in the legal notice.

**Cookies.** `#cookies` lists everything the site keeps in the browser: the
`__Host-hangar_session` cookie (`account.hangarai.dev`, 30 days), the
`hangar_ref` cookie and `hangar.ref` (a referral link's code, 60 days, only
after the visitor's yes on `/r/`, `assets/api.js`), and `hangar.lang`, `hangar.announce`,
`hangar.account.seen`, `hangar.signin` (`assets/lang.js`, `assets/nav.js`,
`account/account.js`). The live site sets no other cookie (checked with its
responses on 2026-11-22; Cloudflare could add `cf_clearance` on a challenge).
A script that stores something new in the browser needs its line there
before it ships. There is no consent banner. The referral cookie and `hangar.ref` are
the ones the CNIL does not count as strictly necessary (its cookie FAQ: the
trackers of affiliation are not exempt from consent), so `/r/<code>` asks
first, and only there (the person's choice, 2026-11-23): « Keep this
invitation? », Accept and Refuse as two plain `.btn` of the same size, a link
to `privacy.html#cookies`, and one line on what refusing costs (no bonus
credits at a checkout of the site, no one credited; the code can still be typed
in the app). Nothing is stored before Accept (`assets/invite.js` calls
`referral.remember` only then); Refuse stores nothing, the refusal included, so
the next link asks again. `assets/api.js` keeps `{code, at, consent}` and
removes on sight a `hangar.ref` without `consent` or past its 60 days, and a
`hangar_ref` cookie alone where the storage works: both are what the site
kept before it asked. The consent is withdrawn on `/r/` without a code, which
shows the invitation held with « Remove the invitation » (the link
`privacy.html` gives), or by clearing the site's data. Without a code kept, a
checkout of the site carries none and works the same. The visit a link counts
on the account server (`POST /v1/referral/visits`) is made before the answer:
it stores nothing in the browser, and the server keeps only the day's hash of
the address (`crates/hangar-cloud/src/referral/landing.rs`). Nothing else
that is not strictly necessary may be added without consent.

**Not checked by a lawyer.** `legal/terms-app.html`, `terms/` and
`legal/affiliate-terms.html` say so at their top (`.legal__draft`); remove the
note once a reviewed version replaces the text. Choices that are the
person's, written so that they are easy to change: French law; for
professionals the courts of Saint-Étienne, consumers keeping their own
(`legal/terms-app.html#disputes`); the sale is Polar's as merchant of record,
which collects the request for immediate supply and the acknowledgment of
losing the right of withdrawal (`#withdrawal`); refunds as `terms/#credits`
says; the free license of 2.x installs, held by the HangarAI account, on up
to 3 computers like Pro and Max (`#license`, `#licenses`, `pricing/`'s
questions, `privacy.html#license`); an account from 16, buying from 18 or with a parent's permission
(`#age`, and `privacy.html#rights`).

## Releasing a new version

The download links name no version. Each one goes to
`https://github.com/hangar-ai/hangar/releases/latest/download/<name>`, a file
`scripts/release/publish` puts on every release a second time under a name
without the version (`scripts/release/hangar_release.py downloads` lists them),
so it is always the latest release's:

```
Hangar.AI_x64-setup.exe
Hangar.AI_universal.dmg
Hangar.AI_amd64.AppImage
Hangar.AI_amd64.deb
Hangar.AI_x86_64.rpm
```

Never link a version's own file (`/releases/download/v<v>/…`): a major release
deletes every earlier release, with its files and its tag
(`docs/native/PACKAGING.md`, "Retiring the earlier releases"), and the link
would lead nowhere. `node scripts/site-shots/links.mjs` refuses one, and any
other link to the releases than their page, `/releases/latest` or one of these
files. Those files exist from the first release `publish` makes with them
(3.0.0) on, which is one more reason the site goes out after the release. A
release published with `--skip-macos` has no `.dmg`: the macOS link has nothing
to give until the next one that has it.

The version is still written in the page as text: replace it in the download
heading (`3.0` now), the JSON-LD's `softwareVersion` (`3.0.0`) and the share
titles, which name the minor version. Forgetting is not fatal: the app offers each
new version once installed, and "Every file of the latest release" always
points at `/releases/latest`.

## Deploy

Static, served by [static-web-server](https://static-web-server.net) behind
Cloudflare. The `Dockerfile` at the root copies `site/` as it is into the
server's image; `sws.toml` is its configuration: compression, and on every
response `X-Content-Type-Options: nosniff`, `X-Frame-Options: SAMEORIGIN` (no
other site may frame the page), a referrer policy and a permissions policy.

static-web-server picks the `Content-Type` from the extension, and gets every one
the page uses right (`image/avif`, `image/webp`, `font/woff2`, `image/x-icon`,
`image/svg+xml`), which `nosniff` makes load-bearing: a wrong one and the browser
refuses the file. After a change of server or of image format, check them:

```bash
curl -sI https://hangarai.dev/img/hero-1280.avif | grep -i content-type
```

The canonical address is https://hangarai.dev, which the canonical link and
the Open Graph tags name.

The film is `video/mp4`, served in ranges (static-web-server answers
`Range`), so a browser can start it before it has it all:

```bash
curl -sI https://hangarai.dev/video/hangarai-3.0-en-720p.mp4 | grep -i -E 'content-type|accept-ranges'
```
