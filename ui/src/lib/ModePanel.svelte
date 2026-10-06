<script lang="ts">
  import { MODES, type Action, type Mode } from "../types";

  interface Props {
    mode: Mode | null;
    disabled?: boolean;
    send: (action: Action) => void;
  }
  let { mode, disabled = false, send }: Props = $props();
</script>

<section class="panel" aria-label="Mode">
  <h2>Mode</h2>
  <div class="grid">
    {#each MODES as m}
      <button
        class:active={m === mode}
        aria-pressed={m === mode}
        {disabled}
        onclick={() => send({ type: "set_mode", mode: m })}>{m}</button
      >
    {/each}
  </div>
</section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(108px, 1fr));
    gap: 8px;
  }
  button.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
    font-weight: 700;
  }
</style>
