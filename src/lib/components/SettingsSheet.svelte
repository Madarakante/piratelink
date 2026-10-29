<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    pickEmulator,
    revealData,
    saveSettings,
    type GlyphChoice,
    type HeroFit,
    type Settings,
    type SettingsPatch,
  } from "../library";

  let {
    open,
    settings,
    onClose,
    onSaved,
    onRefreshAll,
  }: {
    open: boolean;
    settings: Settings | null;
    onClose: () => void;
    onSaved: (next: Settings) => void;
    onRefreshAll: (progress: (done: number, total: number) => void) => Promise<void>;
  } = $props();

  /** Typed key; empty until the user types. Never prefilled with the real one. */
  let keyInput = $state("");
  let busy = $state<"" | "key" | "emulator" | "option">("");
  let error = $state("");
  let refresh = $state<{ done: number; total: number } | null>(null);

  function mb(bytes: number): string {
    if (!bytes) return "0 MB";
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  }

  async function patch(next: SettingsPatch, kind: "key" | "emulator" | "option" = "option") {
    busy = kind;
    error = "";
    try {
      const saved = await saveSettings(next);
      onSaved(saved);
      if (next.steamgriddb_api_key !== undefined) keyInput = "";
    } catch (err) {
      error = String(err);
    } finally {
      busy = "";
    }
  }

  async function chooseEmulator() {
    try {
      const path = await pickEmulator();
      if (path) await patch({ ryujinx_path: path }, "emulator");
    } catch (err) {
      error = String(err);
    }
  }

  async function refreshAll() {
    if (refresh) return;
    error = "";
    refresh = { done: 0, total: 0 };
    try {
      await onRefreshAll((done, total) => (refresh = { done, total }));
    } catch (err) {
      error = String(err);
    } finally {
      refresh = null;
    }
  }

  const heroFit = (value: HeroFit) => patch({ hero_fit: value });
  const glyphs = (value: GlyphChoice) => patch({ glyphs: value });
  const sound = (value: boolean) => patch({ sound: value });
</script>

<!--
  Settings.

  A printed index, not a control panel: one column of hairline rows, the name on
  the left, the control on the right, and the path worth checking printed under
  the row in monospace. Everything here writes to config.json immediately - a
  Save button that could be forgotten is worse than a write per click.
-->
{#if open}
  <div
    class="wrap"
    role="dialog"
    aria-modal="true"
    aria-label="Settings"
    transition:fade={{ duration: 160 }}
  >
    <header>
      <span class="label">Settings</span>
      <span class="target">
        {#if settings}
          PirateLink {settings.version} · {settings.games} titles
        {/if}
      </span>
      <button class="quiet" onclick={onClose}>Close - Esc</button>
    </header>

    <div class="body">
      <div class="column">
        {#if error}
          <p class="error num">{error}</p>
        {/if}

        <section>
          <h2 class="label">Artwork</h2>

          <div class="row">
            <span class="name">SteamGridDB key</span>
            <div class="ctl">
              <input
                class="field"
                type="password"
                bind:value={keyInput}
                placeholder={settings?.api_key_set ? `saved · ending ${settings.api_key_tail}` : "paste your API key"}
                spellcheck="false"
                autocomplete="off"
                aria-label="SteamGridDB API key"
              />
              <div class="acts">
                <button
                  class="btn"
                  disabled={!keyInput.trim() || busy === "key"}
                  onclick={() => patch({ steamgriddb_api_key: keyInput }, "key")}
                >
                  {busy === "key" ? "Saving" : "Save"}
                </button>
                {#if settings?.api_key_set}
                  <button
                    class="quiet"
                    disabled={busy === "key"}
                    onclick={() => patch({ steamgriddb_api_key: "" }, "key")}
                  >
                    Clear
                  </button>
                {/if}
              </div>
            </div>
          </div>
          <p class="note">
            {#if settings?.api_key_set}
              Lookup is on. The key stays in config.json and is never sent to the page.
            {:else}
              Without a key, adding a game still works but no cover is fetched.
            {/if}
          </p>

          <div class="row">
            <span class="name">Hero art</span>
            <div class="ctl">
              <div class="seg">
                <button class:on={settings?.hero_fit === "fit"} onclick={() => heroFit("fit")}>
                  Whole image
                </button>
                <button class:on={settings?.hero_fit === "fill"} onclick={() => heroFit("fill")}>
                  Fill window
                </button>
              </div>
            </div>
          </div>
          <p class="note">
            Whole image prints the hero at its own 3:1 shape above the shelf. Fill crops
            it to the window instead.
          </p>

          <div class="row">
            <span class="name">Cached artwork</span>
            <div class="ctl">
              <span class="value num">
                {settings ? `${settings.art_files} files · ${mb(settings.art_bytes)}` : "-"}
              </span>
              <div class="acts">
                <button class="btn" disabled={!!refresh || !settings?.games} onclick={refreshAll}>
                  {#if refresh}
                    {refresh.done} / {refresh.total || settings?.games}
                  {:else}
                    Refresh all
                  {/if}
                </button>
              </div>
            </div>
          </div>
          <p class="note">
            Re-runs the automatic match for every title, which is how older 1080p heroes
            get replaced by the 4K copy.
          </p>
        </section>

        <section>
          <h2 class="label">Switch</h2>

          <div class="row">
            <span class="name">Emulator</span>
            <div class="ctl">
              <span class="value path" title={settings?.ryujinx_path || settings?.ryujinx_found || ""}>
                {settings?.ryujinx_path || "Auto-detect"}
              </span>
              <div class="acts">
                <button class="btn" disabled={busy === "emulator"} onclick={chooseEmulator}>
                  Choose…
                </button>
                {#if settings?.ryujinx_path}
                  <button
                    class="quiet"
                    disabled={busy === "emulator"}
                    onclick={() => patch({ ryujinx_path: "" }, "emulator")}
                  >
                    Auto
                  </button>
                {/if}
              </div>
            </div>
          </div>
          <p class="note">
            {#if settings?.ryujinx_found}
              Found <code>{settings.ryujinx_found}</code> - ROMs open as
              <code>Ryujinx.exe "&lt;rom&gt;"</code>.
            {:else}
              No Ryujinx found. Choose its executable, or install it and use Auto.
            {/if}
          </p>
        </section>

        <section>
          <h2 class="label">Play</h2>

          <div class="row">
            <span class="name">UI sounds</span>
            <div class="ctl">
              <div class="seg">
                <button class:on={settings?.sound} onclick={() => sound(true)}>On</button>
                <button class:on={settings && !settings.sound} onclick={() => sound(false)}>
                  Off
                </button>
              </div>
            </div>
          </div>

          <div class="row">
            <span class="name">Controller glyphs</span>
            <div class="ctl">
              <div class="seg">
                <button class:on={settings?.glyphs === "auto"} onclick={() => glyphs("auto")}>
                  Auto
                </button>
                <button class:on={settings?.glyphs === "xbox"} onclick={() => glyphs("xbox")}>
                  Xbox
                </button>
                <button
                  class:on={settings?.glyphs === "playstation"}
                  onclick={() => glyphs("playstation")}
                >
                  PlayStation
                </button>
              </div>
            </div>
          </div>
          <p class="note">
            Auto follows the pad that is connected; the other two label the hint bar
            for a specific one.
          </p>
        </section>

        <section>
          <h2 class="label">Storage</h2>

          {#each [
            { kind: "database", name: "Library", path: settings?.database ?? "", action: "data" },
            { kind: "art", name: "Artwork cache", path: settings?.art_dir ?? "", action: "art" },
            { kind: "log", name: "Command log", path: settings?.log_path ?? "", action: "log" },
            { kind: "config", name: "Configuration", path: settings?.config_path ?? "", action: "config" },
          ] as entry (entry.kind)}
            <div class="row tight">
              <span class="name">{entry.name}</span>
              <div class="ctl">
                <span class="value path" title={entry.path}>{entry.path || "-"}</span>
                <div class="acts">
                  <button
                    class="quiet"
                    disabled={!entry.path}
                    onclick={() => revealData(entry.action as "data" | "art" | "log" | "config")}
                  >
                    Show
                  </button>
                </div>
              </div>
            </div>
          {/each}
        </section>

        <section class="about">
          <h2 class="label">About</h2>
          <p class="note">
            Artwork by SteamGridDB. UI sounds are generated by
            <code>tools/make_assets.py</code>. No telemetry, no update pings.
          </p>
          <p class="keys label">
            <b>← →</b> browse <b>Enter</b> play <b>Esc</b> back <b>M</b> mute <b>S</b> settings
          </p>
        </section>
      </div>
    </div>

    <footer>
      <span class="label">config.json</span>
      <span class="note">Changes apply immediately and are written to disk.</span>
    </footer>
  </div>
{/if}

<style>
  .wrap {
    position: fixed;
    top: var(--rail);
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 60;
    display: grid;
    grid-template-rows: 52px minmax(0, 1fr) 44px;
    padding: 0 var(--gutter);
    background: var(--bg-0);
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 18px;
    border-bottom: 1px solid var(--line);
  }

  footer {
    justify-content: space-between;
    border-top: 1px solid var(--line);
    border-bottom: 0;
  }

  .target {
    flex: 1;
    overflow: hidden;
    color: var(--ink-dim);
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .body {
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .column {
    max-width: 920px;
    padding: 26px 0 34px;
  }

  section {
    padding-top: 18px;
  }

  section + section {
    margin-top: 22px;
    border-top: 1px solid var(--line);
  }

  section h2 {
    margin: 0 0 4px;
    color: var(--ink-ghost);
  }

  .row {
    display: grid;
    grid-template-columns: 186px minmax(0, 1fr);
    align-items: center;
    gap: 20px;
    min-height: 44px;
    border-top: 1px solid var(--line);
    margin-top: 10px;
  }

  .row.tight {
    min-height: 34px;
    margin-top: 0;
  }

  .row .name {
    color: var(--ink-dim);
    font-size: 12.5px;
    font-weight: 500;
  }

  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .value {
    color: var(--ink-faint);
    font-size: 11px;
  }

  /* Every action lines up on the right edge, whatever the value's length. */
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    flex: none;
  }

  .path {
    overflow: hidden;
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .field {
    flex: 1;
    min-width: 0;
    max-width: 320px;
    height: 30px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: 0;
    background: rgba(255, 255, 255, 0.03);
    color: var(--ink);
    font: 500 11.5px/1 var(--font-mono);
  }

  .field::placeholder {
    color: var(--ink-ghost);
  }

  .field:focus {
    outline: none;
    border-color: var(--line-strong);
  }

  .btn {
    height: 28px;
    padding: 0 11px;
    border: 1px solid var(--line-strong);
    border-radius: 0;
    background: transparent;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    cursor: pointer;
    transition:
      background var(--dur-1) linear,
      border-color var(--dur-1) linear;
  }

  .btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.07);
  }

  .btn:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .quiet {
    height: 28px;
    padding: 0 8px;
    border: 0;
    background: none;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .quiet:hover:not(:disabled) {
    color: var(--ink);
  }

  .quiet:disabled {
    cursor: default;
    opacity: 0.4;
  }

  .seg {
    display: flex;
  }

  .seg button {
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-right-width: 0;
    background: transparent;
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .seg button:last-child {
    border-right-width: 1px;
  }

  .seg button:hover {
    color: var(--ink);
  }

  .seg button.on {
    background: var(--ink);
    border-color: var(--ink);
    color: #07080a;
  }

  .note {
    margin: 7px 0 0;
    color: var(--ink-ghost);
    font-size: 11px;
    line-height: 1.65;
    max-width: 78ch;
  }

  .note code {
    color: var(--ink-faint);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .error {
    margin: 0 0 18px;
    color: #ff9b8a;
    font-size: 11px;
  }

  .about .keys {
    margin: 12px 0 0;
    color: var(--ink-ghost);
  }

  .about .keys b {
    margin-right: 4px;
    color: var(--ink-faint);
    font-weight: 500;
  }

  .about .keys b:not(:first-child) {
    margin-left: 12px;
  }

  @media (max-width: 900px) {
    .row {
      grid-template-columns: 1fr;
      gap: 8px;
      padding: 10px 0;
    }
    .column {
      padding-bottom: 24px;
    }
  }
</style>
