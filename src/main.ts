import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";

/**
 * A Tauri window with a broken frontend is just a black rectangle, which is
 * indistinguishable from "the app hung". Anything that prevents the first mount
 * is rendered on screen instead.
 */
let mounted = false;

function fatal(message: string) {
  if (mounted) return;
  const root = document.getElementById("app");
  if (!root) return;
  root.textContent = "";
  const panel = document.createElement("div");
  panel.className = "fatal";
  panel.textContent = message;
  root.appendChild(panel);
}

window.addEventListener("error", (e) => fatal(`Startup error: ${e.message}`));
window.addEventListener("unhandledrejection", (e) => fatal(`Startup error: ${String(e.reason)}`));

try {
  mount(App, { target: document.getElementById("app")! });
  mounted = true;
} catch (err) {
  fatal(`Startup error: ${String(err)}`);
}
