<script lang="ts">
  import { reveal } from './motion';

  export let title: string;
  export let tone: string = 'blue';
</script>

<section class={`panel tone-${tone}`} use:reveal={{ y: 4 }}>
  <h2>{title}</h2>
  <div class="panel-body">
    <slot />
  </div>
</section>
