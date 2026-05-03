<script lang="ts">
  import UsageConsole from '../components/UsageConsole.svelte';
  import { reveal } from '../motion';
  import type { DesktopSnapshot } from '../types';

  export let snapshot: DesktopSnapshot;
  export let usageTone: (tool: string, index: number) => string;
</script>

<section class="page usage-page" use:reveal={{ y: 5 }}>
  <section class="usage-grid">
    {#each snapshot.usage.sections as section, index}
      <UsageConsole {section} tone={usageTone(section.tool, index)} copy={snapshot.copy} />
    {/each}
  </section>
</section>
