<script lang="ts">
  import { animatedBar } from '../motion';

  export let used = 0;
  export let ariaLabel = '';
  export let usedSuffix = 'used';

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(used) ? used : 0));
  $: tone = clamped >= 88 ? 'hot' : clamped >= 60 ? 'warm' : 'cool';
</script>

<span class="gauge-bar" aria-label={`${ariaLabel}: ${Math.round(clamped)}% ${usedSuffix}`}>
  <span class={`gauge-fill ${tone}`} use:animatedBar={{ value: clamped }}></span>
</span>

<style>
  .gauge-bar {
    display: block;
    width: 100%;
    min-width: 74px;
    height: 10px;
    background: var(--color-bar-empty);
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    overflow: hidden;
  }

  .gauge-fill {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: var(--radius-pill);
  }

  .gauge-fill.cool {
    background: var(--color-cyan);
  }

  .gauge-fill.warm {
    background: var(--color-warning);
  }

  .gauge-fill.hot {
    background: var(--color-error);
  }
</style>
