<script lang="ts">
  import type { CopyDeck, ShortcutHint } from '../types';

  export let copy: CopyDeck;
  export let source: 'live' | 'sample';
  export let currency: string;
  export let hints: ShortcutHint[];
  export let sortLabel: string;
</script>

<footer class="status-bar">
  <div class="status-left">
    <span class="source-pill" class:sample={source === 'sample'}>
      <i aria-hidden="true"></i>
      {source}
    </span>
    <span class="mono">{currency}</span>
  </div>
  <div class="status-right">
    {#each hints as hint}
      <span><b>{hint.keys}</b> {hint.action === 'cycle_sort' ? `${copy.filters.sort} ${sortLabel}` : hint.label}</span>
    {/each}
  </div>
</footer>
