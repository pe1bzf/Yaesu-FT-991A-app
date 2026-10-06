<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./api";
  import FilterPanel from "./lib/FilterPanel.svelte";
  import ModePanel from "./lib/ModePanel.svelte";
  import Vfo from "./lib/Vfo.svelte";
  import type { Action, PortInfo, RadioState } from "./types";

  const SIMULATOR = "__sim__";
  const BAUDS = [4800, 9600, 19200, 38400];

  let ports = $state<PortInfo[]>([]);
  let choice = $state(SIMULATOR);
  let baud = $state(38400);
  let radio = $state<RadioState | null>(null);
  let usingSim = $state(false);
  let busy = $state(false);
  let error = $state("");

  const live = $derived(radio !== null && radio.connected);

  onMount(() => {
    if (!api.available) return;
    refreshPorts();
    const unlisten = api.onState((s) => {
      // Na "verbreken" kunnen er nog late meldingen binnenkomen.
      if (radio) radio = s;
    });
    return () => void unlisten.then((f) => f());
  });

  async function refreshPorts() {
    try {
      ports = await api.listPorts();
      const cat = ports.find((p) => p.likely_cat);
      if (cat && choice === SIMULATOR) choice = cat.name;
    } catch (e) {
      error = String(e);
    }
  }

  async function connect() {
    busy = true;
    error = "";
    try {
      const sim = choice === SIMULATOR;
      radio = await api.connect(
        sim ? { kind: "simulator" } : { kind: "serial", port: choice, baud },
      );
      usingSim = sim;
    } catch (e) {
      error = String(e);
      radio = null;
    } finally {
      busy = false;
    }
  }

  async function disconnect() {
    await api.disconnect();
    radio = null;
    usingSim = false;
  }

  // Alle knoppen sturen hun actie hierheen.
  async function send(action: Action) {
    error = "";
    try {
      radio = await api.act(action);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<main>
  <header>
    <h1>FT-991A</h1>
    <span class="status" class:ok={live} class:lost={radio && !radio.connected}>
      {#if live}
        {usingSim ? "Simulator" : "Verbonden"}
      {:else if radio}
        Verbinding verloren
      {:else}
        Niet verbonden
      {/if}
    </span>
  </header>

  {#if !api.available}
    <p class="notice">
      Deze pagina werkt binnen de app. Start hem met <code>npm run tauri dev</code>.
    </p>
  {/if}

  <section class="panel connect" aria-label="Verbinding">
    <select bind:value={choice} disabled={!!radio || busy} aria-label="Poort">
      <option value={SIMULATOR}>Simulator (zonder radio)</option>
      {#each ports as p}
        <option value={p.name}>
          {p.name}{p.description ? ` · ${p.description}` : ""}{p.likely_cat ? " · waarschijnlijk CAT" : ""}
        </option>
      {/each}
    </select>
    {#if choice !== SIMULATOR}
      <select bind:value={baud} disabled={!!radio || busy} aria-label="Baudrate">
        {#each BAUDS as b}
          <option value={b}>{b} baud</option>
        {/each}
      </select>
    {/if}
    <button onclick={refreshPorts} disabled={!!radio || busy || !api.available} title="Poorten opnieuw zoeken"
      >↻</button
    >
    {#if radio}
      <button onclick={disconnect}>Verbreken</button>
    {:else}
      <button class="primary" onclick={connect} disabled={busy || !api.available}>
        {busy ? "Bezig…" : "Verbinden"}
      </button>
    {/if}
  </section>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if radio}
    <div class="vfos">
      <Vfo vfo="A" hz={radio.freq_a} main disabled={!live} {send} />
      <div class="vfo-ops" aria-label="VFO wisselen en kopiëren">
        <button disabled={!live} onclick={() => send({ type: "swap_vfo" })} title="VFO-A en VFO-B verwisselen (A/B-toets)"
          >A ⇄ B</button
        >
        <button disabled={!live} onclick={() => send({ type: "copy_a_to_b" })} title="VFO-A kopiëren naar VFO-B"
          >A → B</button
        >
        <button disabled={!live} onclick={() => send({ type: "copy_b_to_a" })} title="VFO-B kopiëren naar VFO-A"
          >B → A</button
        >
      </div>
      <Vfo vfo="B" hz={radio.freq_b} disabled={!live} {send} />
    </div>

    <div class="columns">
      <ModePanel mode={radio.mode} disabled={!live} {send} />
      <FilterPanel mode={radio.mode} filter={radio.filter} disabled={!live} {send} />
    </div>

    {#if usingSim}
      <section class="panel sim" aria-label="Simulator">
        <span>Simulator: draai aan de VFO-A-knop</span>
        <button onclick={() => api.simTurnDial(-1000)}>−1 kHz</button>
        <button onclick={() => api.simTurnDial(1000)}>+1 kHz</button>
      </section>
    {/if}
  {/if}
</main>

<style>
  main {
    max-width: 980px;
    margin: 0 auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h1 {
    margin: 0;
    font-size: 20px;
    letter-spacing: 0.04em;
  }
  .status {
    font-size: 13px;
    padding: 4px 10px;
    border-radius: 999px;
    background: var(--line);
    color: var(--muted);
  }
  .status.ok {
    background: var(--ok-bg);
    color: var(--ok);
  }
  .status.lost {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .connect {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .connect > select:first-child {
    flex: 1;
    min-width: 200px;
  }
  .vfos {
    display: grid;
    gap: 12px;
  }
  .vfo-ops {
    display: flex;
    justify-content: center;
    gap: 8px;
  }
  .columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  @media (max-width: 720px) {
    .columns {
      grid-template-columns: 1fr;
    }
  }
  .sim {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    color: var(--muted);
  }
  .sim span {
    margin-right: auto;
  }
  .error {
    margin: 0;
    padding: 8px 12px;
    border-radius: 8px;
    background: var(--danger-bg);
    color: var(--danger);
  }
  .notice {
    margin: 0;
    color: var(--muted);
  }
</style>
