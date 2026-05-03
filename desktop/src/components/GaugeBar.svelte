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
    height: 12px;
    background: #292d42;
    border: 1px solid #414866;
    overflow: hidden;
  }

  .gauge-fill {
    display: block;
    width: 100%;
    height: 100%;
  }

  .gauge-fill.cool {
    background: #4df3e8;
  }

  .gauge-fill.warm {
    background: #ffd60a;
  }

  .gauge-fill.hot {
    background: #ff5f6d;
  }
</style>
