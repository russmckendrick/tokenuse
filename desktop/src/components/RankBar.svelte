<script lang="ts">
  export let value = 0;
  export let ariaLabel = 'relative rank';
  export let compact = false;

  const segmentCount = 10;
  const segments = Array.from({ length: segmentCount });

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(value) ? value : 0));
  $: filled = clamped === 0 ? 0 : Math.max(1, Math.ceil((clamped / 100) * segmentCount));
</script>

<span class="rank-bar" class:compact aria-label={`${ariaLabel}: ${Math.round(clamped)}%`}>
  {#each segments as _, index}
    <span class:filled={index < filled}></span>
  {/each}
</span>

<style>
  .rank-bar {
    display: grid;
    grid-template-columns: repeat(10, minmax(0, 1fr));
    gap: 1px;
    width: 84px;
    height: 12px;
    align-items: stretch;
  }

  .rank-bar.compact {
    width: 68px;
    height: 10px;
  }

  .rank-bar span {
    min-width: 0;
    background: #292d42;
    border: 1px solid #414866;
  }

  .rank-bar span.filled:nth-child(1),
  .rank-bar span.filled:nth-child(2) {
    background: #62a6ff;
    border-color: #62a6ff;
  }

  .rank-bar span.filled:nth-child(3),
  .rank-bar span.filled:nth-child(4) {
    background: #7ebcff;
    border-color: #7ebcff;
  }

  .rank-bar span.filled:nth-child(5),
  .rank-bar span.filled:nth-child(6) {
    background: #f5cf6c;
    border-color: #f5cf6c;
  }

  .rank-bar span.filled:nth-child(7),
  .rank-bar span.filled:nth-child(8) {
    background: #ffd60a;
    border-color: #ffd60a;
  }

  .rank-bar span.filled:nth-child(9) {
    background: #ff9c48;
    border-color: #ff9c48;
  }

  .rank-bar span.filled:nth-child(10) {
    background: #ff5f6d;
    border-color: #ff5f6d;
  }
</style>
