<script lang="ts">
  import { formatHz } from "../format";
  import type { Action, FilterState, Mode } from "../types";

  interface Props {
    mode: Mode | null;
    filter: FilterState | null;
    disabled?: boolean;
    send: (action: Action) => void;
  }
  let { mode, filter, disabled = false, send }: Props = $props();

  // Lokale waarde tijdens het slepen; pas bij loslaten naar de radio.
  let shift = $state(0);
  $effect(() => {
    shift = filter?.if_shift_hz ?? 0;
  });

  function optionLabel(index: number, hz: number): string {
    return `${String(index).padStart(2, "0")} · ${formatHz(hz)}`;
  }
</script>

<section class="panel" aria-label="Filters">
  <h2>Filter</h2>

  {#if !filter}
    <p class="muted">
      {#if mode}
        In {mode} is de bandbreedte vast. Kies FM-N of AM-N voor smal.
      {:else}
        Nog geen mode bekend.
      {/if}
    </p>
  {:else}
    <div class="row">
      <label for="width">Bandbreedte</label>
      <select
        id="width"
        {disabled}
        value={filter.width_index}
        onchange={(e) => send({ type: "set_width", index: Number(e.currentTarget.value) })}
      >
        {#each filter.width_options as o}
          <option value={o.index}>{optionLabel(o.index, o.hz)}</option>
        {/each}
      </select>
      <span class="value">{filter.width_hz ? formatHz(filter.width_hz) : "?"}</span>
    </div>

    <div class="row">
      <span class="label">Narrow</span>
      <button
        class="toggle"
        class:on={filter.narrow}
        aria-pressed={filter.narrow}
        {disabled}
        onclick={() => send({ type: "set_narrow", on: !filter.narrow })}
        >{filter.narrow ? "Aan" : "Uit"}</button
      >
    </div>

    <div class="row">
      <label for="shift">IF-shift</label>
      <input
        id="shift"
        type="range"
        min="-1200"
        max="1200"
        step="20"
        {disabled}
        bind:value={shift}
        onchange={() => send({ type: "set_if_shift", hz: shift })}
      />
      <span class="value">{shift > 0 ? "+" : ""}{shift} Hz</span>
      <button
        {disabled}
        onclick={() => send({ type: "set_if_shift", hz: 0 })}
        title="IF-shift op 0">0</button
      >
    </div>
  {/if}
</section>

<style>
  .row {
    display: grid;
    grid-template-columns: 100px 1fr auto auto;
    gap: 10px;
    align-items: center;
    margin-bottom: 10px;
  }
  .row:last-child {
    margin-bottom: 0;
  }
  .row > select,
  .row > .toggle {
    grid-column: 2 / span 1;
  }
  .value {
    font-family: var(--mono);
    min-width: 72px;
    text-align: right;
  }
  .toggle {
    justify-self: start;
    min-width: 72px;
  }
  .toggle.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .muted {
    color: var(--muted);
    margin: 0;
  }
</style>
