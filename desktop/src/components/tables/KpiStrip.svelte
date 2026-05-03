<script lang="ts">
  import { staggeredReveal } from '../../motion';
  import type { CopyDeck, Summary } from '../../types';

  export let summary: Summary;
  export let currency: string;
  export let copy: CopyDeck;
</script>

<section class="kpis" use:staggeredReveal={{ selector: ':scope > div', y: 4, stagger: 0.025 }}>
  <div><span>{copy.metrics.cost}</span><strong>{summary.cost}</strong><small>{currency}</small></div>
  <div><span>{copy.metrics.calls}</span><strong>{summary.calls}</strong><small>{summary.input} {copy.metrics.in}</small></div>
  <div><span>{copy.metrics.sessions}</span><strong>{summary.sessions}</strong><small>{copy.metrics.active_set}</small></div>
  <div><span>{copy.metrics.cache_hit}</span><strong>{summary.cache_hit}</strong><small>{summary.cached} {copy.metrics.cached}</small></div>
  <div><span>{copy.metrics.in} / {copy.metrics.out}</span><strong>{summary.input}</strong><small>{summary.output} {copy.metrics.out}</small></div>
</section>
