<script lang="ts">
  import { countUp, staggeredReveal } from '../../motion';
  import type { CopyDeck, Summary } from '../../types';

  export let summary: Summary;
  export let currency: string;
  export let copy: CopyDeck;

  $: callsSub = `${summary.input} ${copy.metrics.in}`;
  $: cacheSub = `${summary.cached} ${copy.metrics.cached}`;
  $: outSub = `${summary.output} ${copy.metrics.out}`;
</script>

<section class="kpis" use:staggeredReveal={{ selector: ':scope > div', y: 4, stagger: 0.025 }}>
  <div>
    <span>{copy.metrics.cost}</span>
    <strong use:countUp={summary.cost}>{summary.cost}</strong>
    <small>{currency}</small>
  </div>
  <div>
    <span>{copy.metrics.calls}</span>
    <strong use:countUp={summary.calls}>{summary.calls}</strong>
    <small use:countUp={callsSub}>{callsSub}</small>
  </div>
  <div>
    <span>{copy.metrics.sessions}</span>
    <strong use:countUp={summary.sessions}>{summary.sessions}</strong>
    <small>{copy.metrics.active_set}</small>
  </div>
  <div>
    <span>{copy.metrics.cache_hit}</span>
    <strong use:countUp={summary.cache_hit}>{summary.cache_hit}</strong>
    <small use:countUp={cacheSub}>{cacheSub}</small>
  </div>
  <div>
    <span>{copy.metrics.in} / {copy.metrics.out}</span>
    <strong use:countUp={summary.input}>{summary.input}</strong>
    <small use:countUp={outSub}>{outSub}</small>
  </div>
</section>
