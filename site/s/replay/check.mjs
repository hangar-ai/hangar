#!/usr/bin/env node
// The replay page's own check, with Node alone (no package, nothing built):
//
//     node site/s/replay/check.mjs                  check the page and its data
//     node site/s/replay/check.mjs --page out.html  also write the page made of sample.json
//     node site/s/replay/check.mjs --page out.html --data some.json
//
// It runs replay.js's plain half (the data checked, the board at a moment,
// the time compressed) on sample.json and on broken copies of it, plays the
// board to its end and compares it with the totals, and reads index.html,
// replay.css and replay.js for what a shared page may not hold. The page it
// writes is made as hangar-core's replay::page makes it (the same three
// marked lines replaced), for the mock share (mock.mjs) and a look in a
// browser; the Rust tests check the app's own.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const HERE = dirname(fileURLToPath(import.meta.url));
const read = (name) => readFileSync(join(HERE, name), "utf8");

export const STYLE_LINE = '<link rel="stylesheet" href="replay.css"><!-- @inline -->';
export const SCRIPT_LINE = '<script src="replay.js"></script><!-- @inline -->';
export const DATA_LINE = '<script id="replay-data" type="application/json">null</script><!-- @data -->';
export const TITLE_LINE = "<title>Replay · HangarAI</title>";
export const FONT_URL = "url(../../fonts/inter-latin-opsz.woff2)";

/** replay.css with the site's Inter inside it, as replay::page puts it. */
export function style() {
  const font = readFileSync(join(HERE, "..", "..", "fonts", "inter-latin-opsz.woff2")).toString("base64");
  return read("replay.css").replace(FONT_URL, `url(data:font/woff2;base64,${font})`);
}

/** replay.js's plain half, as it exports itself where there is no document. */
export function renderer() {
  const sandbox = { module: { exports: {} }, Intl, console };
  vm.runInNewContext(read("replay.js"), sandbox, { filename: "replay.js" });
  return sandbox.module.exports;
}

/** JSON a <script> holds safely, as replay::script_json writes it. */
export function scriptJson(data) {
  return JSON.stringify(data)
    .replace(/</g, "\\u003c")
    .replace(/>/g, "\\u003e")
    .replace(/&/g, "\\u0026")
    .replace(/\u2028/g, "\\u2028")
    .replace(/\u2029/g, "\\u2029");
}

const escapeHtml = (text) =>
  text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The self-contained page of `data`, as replay::page makes it. */
export function page(data) {
  const parts = [
    [TITLE_LINE, `<title>${escapeHtml(data.title)} · HangarAI</title>`],
    [STYLE_LINE, `<style>\n${style()}</style>`],
    [DATA_LINE, `<script id="replay-data" type="application/json">${scriptJson(data)}</script>`],
    [SCRIPT_LINE, `<script>\n${read("replay.js")}</script>`],
  ];
  let rest = read("index.html");
  let out = "";
  for (const [marker, replacement] of parts) {
    const at = rest.indexOf(marker);
    if (at < 0) throw new Error(`index.html lost ${marker}`);
    out += rest.slice(0, at) + replacement;
    rest = rest.slice(at + marker.length);
  }
  return out + rest;
}

let failures = 0;
function check(what, ok, detail) {
  if (ok) return;
  failures += 1;
  console.error(`✗ ${what}${detail ? `: ${detail}` : ""}`);
}

function main() {
  const args = process.argv.slice(2);
  const option = (name) => {
    const at = args.indexOf(name);
    return at >= 0 ? args[at + 1] : null;
  };
  const r = renderer();
  const sample = JSON.parse(read("sample.json"));

  // The data.
  check("sample.json is a replay", r.invalid(sample) === null, r.invalid(sample));
  const broken = [
    ["another version", (d) => { d.v = 2; }],
    ["events out of order", (d) => { d.events.reverse(); }],
    ["an event of no card", (d) => { d.events[0].task = "t999"; }],
    ["an agent nobody lists", (d) => { d.events.find((e) => e.agent).agent = "Mallory"; }],
    ["an unknown kind", (d) => { d.events[0].type = "deleted"; }],
    ["a percent over 100", (d) => { d.events.find((e) => e.type === "progress").percent = 140; }],
    ["a picture outside shots/", (d) => { d.events.find((e) => e.type === "landed").pictures = ["../secret.png"]; }],
    ["a picture on another site", (d) => { d.events.find((e) => e.type === "landed").pictures = ["https://example.com/a.png"]; }],
    ["a referral that is no code", (d) => { d.referral = "a b"; }],
    ["a summary that is not text", (d) => { d.events.find((e) => e.type === "landed").summary = { html: "<b>" }; }],
    ["two cards with one id", (d) => { d.tasks[1].id = d.tasks[0].id; }],
    ["a missing total", (d) => { delete d.totals.landed; }],
  ];
  for (const [what, spoil] of broken) {
    const copy = structuredClone(sample);
    spoil(copy);
    check(`refused: ${what}`, r.invalid(copy) !== null);
  }

  // The time.
  const m = r.model(sample);
  const at = m.at;
  check("one position an event", at.length === sample.events.length);
  check(
    "positions go forward, inside the bar",
    at.every((x, i) => x > 0 && x < 1 && (i === 0 || x >= at[i - 1])),
    JSON.stringify(at.slice(0, 5)),
  );
  check("nothing has happened at 0", r.reached(m, 0) === 0);
  check("everything has happened at 1", r.reached(m, 1) === sample.events.length);
  const middle = r.timeAt(m, 0.5);
  check("the clock is within the replay", middle >= sample.from && middle <= sample.to);

  // The board, played to its end, says what the totals say.
  const end = r.stateAt(m, sample.events.length);
  const t = sample.totals;
  check("every card is on the board at the end", end.shown === t.tasks, `${end.shown} of ${t.tasks}`);
  check("landed", end.counts.landed === t.landed, `${end.counts.landed} ≠ ${t.landed}`);
  check("done", end.done === t.done, `${end.done} ≠ ${t.done}`);
  check("sent back", end.counts.failed === t.failed);
  check("lines added", end.counts.added === t.added);
  check("lines removed", end.counts.removed === t.removed);
  check("files", end.counts.files === t.files);
  check("the duration", sample.to - sample.from === t.duration);
  // At every step, a card is in one column at most and only moves forward
  // to Done once.
  for (let n = 0; n <= sample.events.length; n++) {
    const state = r.stateAt(m, n);
    const seen = new Set();
    for (const column of Object.keys(state.columns)) {
      for (const task of state.columns[column]) {
        check(`card ${task.id} once on the board`, !seen.has(task.id));
        seen.add(task.id);
      }
    }
  }
  const start = r.stateAt(m, 0);
  check(
    "a card made during the replay is not on the board before",
    start.shown === sample.tasks.filter((c) => c.start).length,
  );

  // The words.
  for (const lang of ["en", "fr"]) {
    const line = r.headline(lang, sample).map((p) => p[0]).join("");
    check(`the ${lang} headline has no gap`, !/\{\w+\}/.test(line), line);
    const en = Object.keys(r.WORDS.en).sort().join();
    check(`${lang} has every word`, Object.keys(r.WORDS[lang]).sort().join() === en);
  }
  check("40 min", r.duration("en", 39.5 * 60000) === "40 min");
  check("3 h 05", r.duration("en", (3 * 60 + 5) * 60000) === "3 h 05");
  check("2 days 4 h", r.duration("en", (52 * 60) * 60000) === "2 days 4 h");
  check("2 jours", r.duration("fr", 48 * 3600000) === "2 jours");

  // What a shared page may not hold.
  const template = read("index.html");
  for (const line of [STYLE_LINE, SCRIPT_LINE, DATA_LINE, TITLE_LINE]) {
    check(`index.html holds ${line} once`, template.split(line).length === 2);
  }
  for (const [name, text] of [["index.html", template], ["replay.css", read("replay.css")], ["replay.js", read("replay.js")]]) {
    check(`${name}: no form`, !/<form\b/i.test(text));
    check(`${name}: no password field`, !/type=["']?password/i.test(text));
    check(`${name}: no @import`, !/@import\b/i.test(text));
    check(`${name}: no file of another origin`, !/(?:src|url)\s*[=(]\s*["']?(?:https?:)?\/\//i.test(text));
  }
  check("replay.css never ends its <style>", !/<\/style/i.test(read("replay.css")));
  check("replay.js never ends its <script>", !/<\/script/i.test(read("replay.js")));
  check("replay.js writes no HTML", !/\.innerHTML\s*=|insertAdjacentHTML|document\.write/.test(read("replay.js")));
  check("replay.js keeps nothing", !/localStorage|sessionStorage|indexedDB|document\.cookie/.test(read("replay.js")));

  check("replay.css names the font once", read("replay.css").split(FONT_URL).length === 2);
  const made = page(sample);
  check("the page holds no marker", !/<!-- @(inline|data) -->/.test(made));
  check("the page carries its font", made.includes("url(data:font/woff2;base64,") && !made.includes(FONT_URL));
  check("the page links nothing beside it", !/href="replay\.css"|src="replay\.js"/.test(made));
  check("the page's data ends nowhere early", !/<\/script/i.test(scriptJson({ s: "</script><b>" })));

  const out = option("--page");
  if (out) {
    const data = option("--data") ? JSON.parse(readFileSync(option("--data"), "utf8")) : sample;
    check("--data is a replay", r.invalid(data) === null, r.invalid(data));
    writeFileSync(out, page(data));
    console.log(`page written: ${out}`);
  }

  if (failures) {
    console.error(`${failures} check(s) failed`);
    process.exit(1);
  }
  console.log(`replay page: ok (${sample.events.length} events, ${sample.tasks.length} cards, ${broken.length} broken copies refused)`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
