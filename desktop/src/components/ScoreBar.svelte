<script lang="ts">
  import { animatedBar } from '../motion';

  export let score = 0;
  export let ariaLabel = '';

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(score) ? score : 0));
  // Inverse of GaugeBar: a HIGH practice score is good.
  $: tone = clamped >= 70 ? 'good' : clamped >= 40 ? 'warn' : 'bad';
</script>

<span class="score-bar" aria-label={`${ariaLabel}: ${Math.round(clamped)}/100`}>
  <span class={`score-fill ${tone}`} use:animatedBar={{ value: clamped }}></span>
</span>

<style>
  .score-bar {
    display: block;
    width: 100%;
    min-width: 74px;
    height: 10px;
    background: var(--color-bar-empty);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    overflow: hidden;
  }

  .score-fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: var(--radius-pill);
  }

  .score-fill.good {
    background: var(--color-tertiary);
  }

  .score-fill.warn {
    background: var(--color-warning);
  }

  .score-fill.bad {
    background: var(--color-error);
  }
</style>
