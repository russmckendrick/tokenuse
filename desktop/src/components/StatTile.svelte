<script lang="ts">
  import { countUp } from '../motion';

  export let value = '';
  export let label = '';
  export let sub = '';
</script>

<div class="stat">
  <span class="stat-value mono" use:countUp={value}>{value}</span>
  <span class="stat-label">{label}</span>
  {#if sub}
    <span class="stat-sub">{sub}</span>
  {/if}
</div>

<style>
  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .stat-value {
    font-size: var(--text-display);
    font-weight: 600;
  }

  .stat-label {
    font-size: var(--text-label);
    color: var(--color-muted-2);
  }

  .stat-sub {
    font-size: var(--text-label);
    color: var(--color-muted);
  }
</style>
