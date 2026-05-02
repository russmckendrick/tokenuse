<script lang="ts">
  export let used = 0;
  export let ariaLabel = 'usage limit';

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(used) ? used : 0));
  $: tone = clamped >= 88 ? 'hot' : clamped >= 60 ? 'warm' : 'cool';
</script>

<span class="gauge-bar" aria-label={`${ariaLabel}: ${Math.round(clamped)}% used`}>
  <span class={`gauge-fill ${tone}`} style={`width: ${clamped}%`}></span>
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
    height: 100%;
    min-width: 2px;
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
