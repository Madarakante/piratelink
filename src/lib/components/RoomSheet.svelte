<script lang="ts">
  import { fade } from "svelte/transition";
  import { detectZeroTier, joinZeroTier, startRoom, stopRoom, type Room } from "../library";

  let {
    open,
    game,
    onClose,
    onStarted,
  }: { open: boolean; game: string; onClose: () => void; onStarted: (room: Room) => void } = $props();

  let networkId = $state("");
  let hostAddress = $state("");
  let mode = $state<"create" | "join" | "open">("create");
  let inviteLink = $state("");
  let joinedInvite = $state<{ network: string; host: string; port: string } | null>(null);
  let busy = $state(false);
  let error = $state("");
  let detection = $state("");
  let joinStatus = $state("");
  let room = $state<Room | null>(null);

  async function create() {
    busy = true;
    error = "";
    try {
      room = await startRoom(game, networkId, hostAddress);
      onStarted(room);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function detect() {
    busy = true;
    error = "";
    detection = "";
    try {
      const result = await detectZeroTier(networkId);
      if (result.network_id) networkId = result.network_id;
      if (result.address) hostAddress = result.address;
      detection = result.message;
      if (result.status !== "OK" || !result.address) {
        error = `${result.message}. Make sure ZeroTier shows OK and has an assigned IP.`;
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function stopActiveRoom() {
    await stopRoom();
    room = null;
    mode = "open";
  }

  function parseInvite() {
    error = "";
    joinedInvite = null;
    try {
      const value = inviteLink.trim();
      if (!value.startsWith("piratelink://join/")) throw new Error("Paste a PirateLink invite link");
      const url = new URL(value.replace("piratelink://", "https://"));
      const network = url.searchParams.get("network") ?? "";
      const host = url.searchParams.get("host") ?? "";
      const port = url.searchParams.get("port") ?? "";
      if (!network || !host || !port) throw new Error("This invite is missing connection details");
      joinedInvite = { network, host, port };
    } catch (err) {
      error = String(err).replace(/^Error: /, "");
    }
  }

  async function joinNetwork() {
    if (!joinedInvite) return;
    busy = true;
    error = "";
    joinStatus = "";
    try {
      joinStatus = await joinZeroTier(joinedInvite.network);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function invite(active: Room): string {
    return `piratelink://join/${active.token}?network=${active.network_id}&host=${active.host_address}&port=${active.port}`;
  }
</script>

{#if open}
  <div class="room-wrap" role="dialog" aria-modal="true" aria-label="Room" transition:fade={{ duration: 160 }}>
    <header>
      <span class="label">Phase 4 / Room</span>
      <span class="target">Local host · manual ZeroTier</span>
      <button class="quiet" onclick={onClose}>Close - Esc</button>
    </header>

    <div class="room-body">
      <div class="modes" role="tablist" aria-label="Room mode">
        <button class:on={mode === "create"} role="tab" aria-selected={mode === "create"} onclick={() => { mode = "create"; error = ""; }}>Create Room</button>
        <button class:on={mode === "join"} role="tab" aria-selected={mode === "join"} onclick={() => { mode = "join"; error = ""; }}>Join Room</button>
        <button class:on={mode === "open"} role="tab" aria-selected={mode === "open"} onclick={() => { mode = "open"; error = ""; }}>Open Rooms</button>
      </div>

      {#if mode === "open"}
        <p class="label">Open rooms</p>
        {#if room}
          <h2>{room.game}</h2>
          <dl>
            <div><dt>Network</dt><dd>{room.network_id}</dd></div>
            <div><dt>Host endpoint</dt><dd>{room.host_address}:{room.port}</dd></div>
            <div><dt>Status</dt><dd>Waiting for players</dd></div>
          </dl>
          <label class="label" for="invite">Invite link</label>
          <textarea id="invite" readonly value={invite(room)}></textarea>
          <div class="actions">
            <button class="btn" onclick={() => (mode = "create")}>Room details</button>
            <button class="quiet danger" onclick={stopActiveRoom}>Stop room</button>
          </div>
          <p class="note">The room stays active while this window is closed. Stop it explicitly when the session is finished.</p>
        {:else}
          <h2>No open rooms</h2>
          <p class="note">Create a room and it will remain available here until you explicitly stop it or exit PirateLink.</p>
          <button class="btn" onclick={() => (mode = "create")}>Create room</button>
        {/if}
      {:else if mode === "create"}
        {#if room}
          <p class="note">A room is already active. Creating another room will replace it.</p>
        {/if}
        <p class="label">Create local room</p>
        <h2>{game}</h2>
        <p class="note">The host listener runs on this PC. ZeroTier stays optional while its UDP path is being fixed.</p>
        {#if error}<p class="error num">{error}</p>{/if}
        <label class="label" for="network">ZeroTier network ID</label>
        <input id="network" class="field" bind:value={networkId} placeholder="743993800f6ed928" spellcheck="false" />
        <label class="label" for="host">Host ZeroTier address</label>
        <input id="host" class="field" bind:value={hostAddress} placeholder="10.39.120.10" />
        <button class="quiet detect" disabled={busy} onclick={detect}>{busy ? "Checking ZeroTier" : "Detect ZeroTier"}</button>
        {#if detection}<p class="note">{detection}</p>{/if}
        <button class="btn" disabled={busy} onclick={create}>{busy ? "Starting" : "Create room"}</button>
      {:else}
        <p class="label">Join a room</p>
        <h2>Connect to a host</h2>
        <p class="note">Paste the invite from the host. First join the displayed network in ZeroTier, then use the host endpoint for the game.</p>
        {#if error}<p class="error num">{error}</p>{/if}
        <label class="label" for="invite-link">Invite link</label>
        <textarea id="invite-link" class="field" bind:value={inviteLink} placeholder="piratelink://join/..." spellcheck="false"></textarea>
        <button class="btn" onclick={parseInvite}>Read invite</button>
        {#if joinedInvite}
          <dl>
            <div><dt>Network</dt><dd>{joinedInvite.network}</dd></div>
            <div><dt>Host endpoint</dt><dd>{joinedInvite.host}:{joinedInvite.port}</dd></div>
            <div><dt>Next step</dt><dd>Join and authorize in ZeroTier</dd></div>
          </dl>
            <button class="btn" disabled={busy} onclick={joinNetwork}>{busy ? "Joining ZeroTier" : "Join ZeroTier network"}</button>
            {#if joinStatus}<p class="note">{joinStatus}. Wait for Central authorization, then check that ZeroTier shows <code>OK</code>.</p>{/if}
            <p class="note">After ZeroTier shows <code>OK</code>, connect to the host endpoint from the game.</p>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .room-wrap { position: fixed; inset: 0; z-index: 80; display: grid; grid-template-rows: var(--rail) 1fr; background: rgba(13, 15, 18, .97); }
  header { display: flex; align-items: center; gap: 18px; padding: 0 var(--gutter); border-bottom: 1px solid var(--line); }
  .target { color: var(--ink-faint); font-family: var(--font-mono); font-size: 10px; }
  header .quiet { margin-left: auto; }
  .room-body { width: min(620px, calc(100vw - 48px)); margin: 9vh auto; display: grid; align-content: start; gap: 14px; }
  h2 { margin: 0 0 12px; font-size: 36px; letter-spacing: -.04em; }
  .field, textarea { width: 100%; box-sizing: border-box; border: 1px solid var(--line-strong); border-radius: 2px; background: rgba(255,255,255,.04); color: var(--ink); padding: 11px 12px; font: 12px var(--font-mono); }
  textarea { min-height: 82px; resize: vertical; }
  .modes { display: flex; border-bottom: 1px solid var(--line); margin-bottom: 24px; }
  .modes button { border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--ink-faint); padding: 0 0 10px; margin-right: 22px; font: 10px var(--font-mono); letter-spacing: .14em; text-transform: uppercase; cursor: pointer; }
  .modes button.on { border-bottom-color: var(--ink); color: var(--ink); }
  dl { margin: 12px 0; border-top: 1px solid var(--line); }
  dl div { display: flex; justify-content: space-between; gap: 20px; padding: 11px 0; border-bottom: 1px solid var(--line); }
  dt, .label { color: var(--ink-faint); font: 10px var(--font-mono); letter-spacing: .14em; text-transform: uppercase; }
  dd { margin: 0; color: var(--ink); font: 11px var(--font-mono); text-align: right; }
  .note { color: var(--ink-faint); font: 11px/1.6 var(--font-mono); }
  .error { color: #ff9a8f; }
  .detect { justify-self: start; }
  .actions { display: flex; align-items: center; gap: 18px; }
  .danger { color: #ff9a8f; }
</style>