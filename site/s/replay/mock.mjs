#!/usr/bin/env node
// A replay shared in mock mode: the real share viewer (site/s/index.html and
// its assets) showing a replay page made of sample.json, with stand-ins for
// the account server and the shares' origin, on this computer only. Nothing
// is published anywhere; nothing is sent to HangarAI's servers.
//
//     node site/s/replay/mock.mjs [--port 21260] [--data some.json]
//
// It listens on three ports from --port (or $HANGAR_PORT_BASE, else 8790):
//
//   port      the site, as static-web-server serves it (every /s/* is the
//             viewer), with the viewer's Content-Security-Policy from
//             sws.toml, the real origins swapped for the two below;
//   port + 1  the account server: GET /v1/shares/<id> answers the share;
//   port + 2  the shares' origin: /<id>/replay.html, sandboxed as
//             Cloudflare's rules sandbox every shared file
//             (docs/native/shares.md).
//
// and prints the viewer's address, with a referral code (?ref=) as the app
// hands out its link.

import { createServer } from "node:http";
import { readFileSync, statSync, existsSync } from "node:fs";
import { dirname, join, normalize, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { page, renderer } from "./check.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const SITE = normalize(join(HERE, "..", ".."));
const REPO = normalize(join(SITE, ".."));
const ID = "MockReplayLumenSample1";
const ENTRY = "replay.html";

const args = process.argv.slice(2);
const option = (name) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : null;
};
const base = Number(option("--port") || process.env.HANGAR_PORT_BASE || 8790);
const SITE_ORIGIN = `http://localhost:${base}`;
const API_ORIGIN = `http://127.0.0.1:${base + 1}`;
const FILES_ORIGIN = `http://127.0.0.1:${base + 2}`;

const data = JSON.parse(readFileSync(option("--data") || join(HERE, "sample.json"), "utf8"));
const why = renderer().invalid(data);
if (why) {
  console.error(`not a replay: ${why}`);
  process.exit(1);
}
const html = Buffer.from(page(data));
const now = Date.now();

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".webp": "image/webp",
  ".avif": "image/avif",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
};

// The viewer's policy, from sws.toml, pointed at the stand-ins.
function viewerPolicy() {
  const toml = readFileSync(join(REPO, "sws.toml"), "utf8");
  const block = toml.split('source = "/s/**"')[1] || "";
  const found = /Content-Security-Policy = "([^"]+)"/.exec(block);
  if (!found) return null;
  return found[1]
    .replaceAll("https://account.hangarai.dev", API_ORIGIN)
    .replaceAll("https://share.hangarai.dev", FILES_ORIGIN);
}
const policy = viewerPolicy();

const site = createServer((req, res) => {
  const url = new URL(req.url, SITE_ORIGIN);
  let path = decodeURIComponent(url.pathname);
  const viewer = path === "/s" || path.startsWith("/s/");
  if (viewer) path = "/s/index.html";
  if (path.endsWith("/")) path += "index.html";
  const file = normalize(join(SITE, path));
  if (!file.startsWith(SITE) || !existsSync(file) || !statSync(file).isFile()) {
    res.writeHead(404, { "Content-Type": "text/plain" });
    res.end("not found");
    return;
  }
  const headers = {
    "Content-Type": TYPES[extname(file)] || "application/octet-stream",
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  };
  if (viewer) {
    if (policy) headers["Content-Security-Policy"] = policy;
    headers["Referrer-Policy"] = "no-referrer";
    headers["X-Frame-Options"] = "DENY";
  }
  res.writeHead(200, headers);
  res.end(readFileSync(file));
});

const account = createServer((req, res) => {
  const cors = {
    "Access-Control-Allow-Origin": SITE_ORIGIN,
    "Access-Control-Allow-Methods": "GET, POST",
    "Access-Control-Allow-Headers": "Content-Type",
    "Cache-Control": "no-store",
  };
  if (req.method === "OPTIONS") {
    res.writeHead(204, cors);
    res.end();
    return;
  }
  const url = new URL(req.url, API_ORIGIN);
  if (req.method === "GET" && url.pathname === `/v1/shares/${ID}`) {
    res.writeHead(200, { ...cors, "Content-Type": "application/json" });
    res.end(
      JSON.stringify({
        id: ID,
        kind: "page",
        title: data.title,
        entry: ENTRY,
        files: [{ path: ENTRY, size: html.length, content_type: "text/html; charset=utf-8" }],
        content_base: `${FILES_ORIGIN}/${ID}/`,
        live_at: now,
        expires_at: now + 30 * 86_400_000,
        abuse: "abuse@hangarai.dev",
      }),
    );
    return;
  }
  if (req.method === "POST" && url.pathname === `/v1/shares/${ID}/report`) {
    res.writeHead(202, { ...cors, "Content-Type": "application/json" });
    res.end(JSON.stringify({ abuse: "abuse@hangarai.dev" }));
    return;
  }
  res.writeHead(404, { ...cors, "Content-Type": "application/json" });
  res.end(JSON.stringify({ error: "not_found", message: "no share has this id" }));
});

const files = createServer((req, res) => {
  const url = new URL(req.url, FILES_ORIGIN);
  if (url.pathname !== `/${ID}/${ENTRY}`) {
    res.writeHead(404, { "Content-Type": "text/plain" });
    res.end("not found");
    return;
  }
  // Rule 3a of docs/native/shares.md, with this computer's viewer.
  res.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8",
    "Content-Security-Policy": `sandbox allow-scripts allow-popups; frame-ancestors ${SITE_ORIGIN}`,
    "X-Content-Type-Options": "nosniff",
    "X-Robots-Tag": "noindex, nofollow, noarchive",
    "Referrer-Policy": "no-referrer",
    "Access-Control-Allow-Origin": SITE_ORIGIN,
    "Cache-Control": "no-store",
  });
  res.end(html);
});

site.listen(base, "127.0.0.1");
account.listen(base + 1, "127.0.0.1");
files.listen(base + 2, "127.0.0.1", () => {
  const code = data.referral ? `&ref=${data.referral}` : "";
  console.log(`viewer: ${SITE_ORIGIN}/s/${ID}?api=${encodeURIComponent(API_ORIGIN)}${code}`);
  console.log(`page alone: ${FILES_ORIGIN}/${ID}/${ENTRY}`);
});
