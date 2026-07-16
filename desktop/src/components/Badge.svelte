<script lang="ts">
  export let label = '';
  export let tone: 'good' | 'warn' | 'bad' | 'neutral' = 'neutral';
</script>

<span class={`badge ${tone}`}>{label}</span>

<style>
  .badge {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 1px 6px;
    border: 1px solid var(--color-border-soft);
    border-radius: var(--radius-pill);
    white-space: nowrap;
    color: var(--color-muted);
  }

  .badge.good {
    color: var(--color-tertiary);
    border-color: var(--color-tertiary);
  }

  .badge.warn {
    color: var(--color-warning);
    border-color: var(--color-warning);
  }

  .badge.bad {
    color: var(--color-error);
    border-color: var(--color-error);
  }
</style>
