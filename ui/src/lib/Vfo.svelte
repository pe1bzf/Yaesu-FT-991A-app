<script lang="ts">
  import { formatFreq, parseFreq } from "../format";
  import type { Action, Vfo } from "../types";

  interface Props {
    vfo: Vfo;
    hz: number;
    main?: boolean;
    disabled?: boolean;
    send: (action: Action) => void;
  }
  let { vfo, hz, main = false, disabled = false, send }: Props = $props();

  const STEPS = [10, 100, 1_000, 10_000, 100_000, 1_000_000];
  let step = $state(1_000);
  let entry = $state("");
  let entryError = $state(false);

  function stepLabel(s: number): string {
    return s >= 1_000_000 ? `${s / 1_000_000} MHz` : s >= 1000 ? `${s / 1000} kHz` : `${s} Hz`;
  }

  function tune(direction: 1 | -1) {
    send({ type: "step_frequency", vfo, delta_hz: direction * step });
  }

  function onWheel(e: WheelEvent) {
    if (disabled) return;
    e.preventDefault();
    tune(e.deltaY < 0 ? 1 : -1);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    const target = parseFreq(entry);
    entryError = target === null;
    if (target !== null) {
      send({ type: "set_frequency", vfo, hz: target });
      entry = "";
    }
  }
</script>

<section class="vfo" class:main aria-label="VFO-{vfo}">
  <div class="head">
    <span class="tag">VFO-{vfo}</span>
    {#if main}<span class="hint">scroll op de cijfers om af te stemmen</span>{/if}
  </div>

  <div class="readout" onwheel={onWheel} title="Scroll om af te stemmen in stappen van {stepLabel(step)}">
    <span class="digits">{formatFreq(hz)}</span>
    <span class="unit">MHz</span>
  </div>

  <div class="controls">
    <button {disabled} onclick={() => tune(-1)} aria-label="Omlaag">−</button>
    <select bind:value={step} aria-label="Stapgrootte">
      {#each STEPS as s}
        <option value={s}>{stepLabel(s)}</option>
      {/each}
    </select>
    <button {disabled} onclick={() => tune(1)} aria-label="Omhoog">+</button>

    <form onsubmit={submit}>
      <input
        bind:value={entry}
        class:error={entryError}
        oninput={() => (entryError = false)}
        placeholder="bv. 145,500"
        inputmode="decimal"
        aria-label="Frequentie in MHz"
        {disabled}
      />
      <button type="submit" {disabled}>Zet</button>
    </form>
  </div>
</section>

<style>
  .vfo {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 12px 16px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }
  .tag {
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.05em;
  }
  .hint {
    font-size: 12px;
    color: var(--muted);
  }
  .readout {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 2px 0 8px;
    cursor: ns-resize;
    user-select: none;
  }
  .digits {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    font-size: clamp(28px, 7vw, 44px);
    color: var(--display);
  }
  .main .digits {
    font-size: clamp(36px, 10vw, 64px);
  }
  .unit {
    color: var(--muted);
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .controls > button {
    width: 44px;
    font-size: 20px;
  }
  form {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
  input {
    width: 130px;
  }
  input.error {
    border-color: var(--danger);
  }
  @media (max-width: 480px) {
    form {
      margin-left: 0;
      width: 100%;
    }
    input {
      flex: 1;
    }
  }
</style>
