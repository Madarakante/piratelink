#!/usr/bin/env node
/**
 * Minimal static file server for previewing the built UI without Tauri.
 *
 * `python -m http.server` is not good enough for this app:
 *   - it is single-threaded, so one slow request (e.g. an <audio> preload) blocks
 *     every other asset;
 *   - it has no Range support, and media elements do use Range requests.
 * Both problems look like application bugs from the browser side.
 *
 * Usage: node tools/serve.mjs [--port 4174] [--root dist]
 */

import { createReadStream, existsSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join, resolve } from "node:path";

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
};

const port = Number(arg("port", "4174"));
const root = resolve(arg("root", "dist"));

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".webp": "image/webp",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".ogg": "audio/ogg",
  ".wav": "audio/wav",
  ".woff2": "font/woff2",
};

/**
 * Mirrors `app.security.csp` in src-tauri/tauri.conf.json.
 *
 * Serving the preview under the same policy is the point: CSP violations are
 * invisible in a browser that is not enforcing them, and the packaged app is the
 * only place they would otherwise show up.
 */
const CSP =
  "default-src 'self'; img-src 'self' data: blob:; media-src 'self' blob:; " +
  "style-src 'self' 'unsafe-inline'; script-src 'self'; " +
  "connect-src 'self' ipc: http://ipc.localhost";

const server = createServer((req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);

  // Strip leading slashes before joining: on Windows `join(root, "/x")` returns
  // the absolute path "\x" and escapes the root entirely.
  let rel = decodeURIComponent(url.pathname).replace(/^[/\\]+/, "");
  if (rel === "" || rel.endsWith("/")) rel += "index.html";

  const file = join(root, rel);

  // Refuse anything that escapes the root.
  if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) {
    res.writeHead(404, { "content-type": "text/plain" });
    res.end("not found");
    return;
  }

  const size = statSync(file).size;
  const type = TYPES[extname(file).toLowerCase()] ?? "application/octet-stream";
  const range = req.headers.range;

  console.log(`${req.method} ${rel} size=${size} range=${range ?? "-"}`);

  const base = {
    "content-type": type,
    "accept-ranges": "bytes",
    "cache-control": "no-store",
    "content-security-policy": CSP,
  };

  if (range) {
    const m = /bytes=(\d*)-(\d*)/.exec(range);
    const start = m && m[1] ? Number(m[1]) : 0;
    const end = m && m[2] ? Math.min(Number(m[2]), size - 1) : size - 1;
    if (start >= size || start > end) {
      res.writeHead(416, { ...base, "content-range": `bytes */${size}` });
      res.end();
      return;
    }
    res.writeHead(206, {
      ...base,
      "content-range": `bytes ${start}-${end}/${size}`,
      "content-length": end - start + 1,
    });
    createReadStream(file, { start, end }).pipe(res);
    return;
  }

  res.writeHead(200, { ...base, "content-length": size });
  createReadStream(file).pipe(res);
});

server.listen(port, "127.0.0.1", () => {
  console.log(`serving ${root} at http://127.0.0.1:${port}`);
});
