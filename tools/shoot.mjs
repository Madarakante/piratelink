#!/usr/bin/env node
/**
 * Screenshot helper for UI verification.
 *
 * Headless Chromium's `--screenshot` fires while CSS/Svelte transitions are still
 * running (virtual time does not advance them), which makes captures look dimmed
 * and mid-animation. This drives Edge over the DevTools Protocol instead: load,
 * wait a real wall-clock delay, optionally press keys, then capture.
 *
 * Usage:
 *   node tools/shoot.mjs --url http://localhost:1420 --out docs/shot.png
 *   node tools/shoot.mjs --out docs/detail.png --keys Enter
 *   node tools/shoot.mjs --out docs/next.png --keys ArrowRight ArrowRight
 */

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { setTimeout as sleep } from "node:timers/promises";

const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PORT = 9333;

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

const url = arg("url", "http://localhost:1420");
const out = arg("out", "docs/shot.png");
const delay = Number(arg("delay", "2200"));
const width = Number(arg("width", "1440"));
const height = Number(arg("height", "900"));
const keysIndex = process.argv.indexOf("--keys");
const keys = keysIndex >= 0 ? process.argv.slice(keysIndex + 1) : [];

const evalIndex = process.argv.indexOf("--eval");
const evalExpr = evalIndex >= 0 ? process.argv[evalIndex + 1] : null;

/** Extra wall-clock wait after the last key, so transitions can settle. */
const settleIndex = process.argv.indexOf("--settle");
const settle = settleIndex >= 0 ? Number(process.argv[settleIndex + 1]) : 0;

const keyMap = {
  ArrowRight: { code: "ArrowRight", key: "ArrowRight", windowsVirtualKeyCode: 39 },
  ArrowLeft: { code: "ArrowLeft", key: "ArrowLeft", windowsVirtualKeyCode: 37 },
  Enter: { code: "Enter", key: "Enter", windowsVirtualKeyCode: 13 },
  Escape: { code: "Escape", key: "Escape", windowsVirtualKeyCode: 27 },
  Space: { code: "Space", key: " ", windowsVirtualKeyCode: 32 },
  KeyN: { code: "KeyN", key: "n", windowsVirtualKeyCode: 78 },
  KeyM: { code: "KeyM", key: "m", windowsVirtualKeyCode: 77 },
  F3: { code: "F3", key: "F3", windowsVirtualKeyCode: 114 },
};

const profile = `${process.env.TEMP}\\edge-cdp-${Date.now()}`;
const headed = process.argv.includes("--headed");

const edge = spawn(
  EDGE,
  [
    ...(headed ? ["--window-position=80,60"] : ["--headless=new", "--disable-gpu", "--hide-scrollbars"]),
    "--no-first-run",
    "--no-default-browser-check",
    "--autoplay-policy=no-user-gesture-required",
    `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${profile}`,
    `--window-size=${width},${height}`,
    "about:blank",
  ],
  { stdio: "ignore", detached: false },
);

async function waitForDevtools() {
  for (let i = 0; i < 100; i++) {
    try {
      const res = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      const targets = await res.json();
      const page = targets.find((t) => t.type === "page");
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch {
      /* not up yet */
    }
    await sleep(120);
  }
  throw new Error("DevTools endpoint never became available");
}

function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let nextId = 1;
  const pending = new Map();
  const logs = [];

  ws.addEventListener("message", (ev) => {
    const msg = JSON.parse(ev.data);

    if (msg.method === "Runtime.consoleAPICalled") {
      const text = (msg.params.args ?? [])
        .map((a) => a.value ?? a.description ?? a.type)
        .join(" ");
      logs.push(`[${msg.params.type}] ${text}`);
    } else if (msg.method === "Runtime.exceptionThrown") {
      logs.push(`[exception] ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
    }

    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message));
      else resolve(msg.result);
    }
  });

  const ready = new Promise((resolve, reject) => {
    ws.addEventListener("open", () => resolve());
    ws.addEventListener("error", () => reject(new Error("CDP socket error")));
  });

  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });

  return { ready, send, logs, close: () => ws.close() };
}

try {
  const cdp = connect(await waitForDevtools());
  await cdp.ready;

  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await cdp.send("Page.navigate", { url });
  // Without focus Chromium defers media preload and throttles paint.
  await cdp.send("Page.bringToFront");
  await sleep(delay);

  for (const key of keys) {
    const k = keyMap[key];
    if (!k) continue;
    await cdp.send("Input.dispatchKeyEvent", { type: "keyDown", ...k });
    await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...k });
    await sleep(700);
  }

  if (settle > 0) await sleep(settle);

  if (evalExpr) {
    const res = await cdp.send("Runtime.evaluate", {
      expression: evalExpr,
      awaitPromise: true,
      returnByValue: true,
    });
    if (res.exceptionDetails) {
      console.error("eval failed:", res.exceptionDetails.text, res.exceptionDetails.exception?.description ?? "");
    } else {
      console.log("eval:", JSON.stringify(res.result?.value, null, 2));
    }

    // Let anything the eval triggered (a click, a transition) settle before capture.
    await sleep(450);
  }

  const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(out, Buffer.from(shot.data, "base64"));
  console.log(`saved ${out} (${width}x${height}, ${keys.length} keys)`);

  if (cdp.logs.length) {
    console.log("--- console ---");
    for (const line of cdp.logs) console.log(line);
  }

  cdp.close();
} finally {
  edge.kill();
}
