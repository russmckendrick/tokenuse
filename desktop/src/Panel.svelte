<script lang="ts">
  import { reveal } from './motion';

  export let title: string;
  export let tone: string = 'blue';
  export let scrollable = false;
</script>

<section class={`panel tone-${tone}`} class:scrollable use:reveal={{ y: 4 }}>
  <h2>
    <slot name="title-icon" />
    {title}
  </h2>
  <div class="panel-body">
    <slot />
  </div>
</section>
