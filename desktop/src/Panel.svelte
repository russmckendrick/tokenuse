<script lang="ts">
  export let title: string;
  export let tone: string = 'blue';
</script>

<section class={`panel tone-${tone}`}>
  <h2>{title}</h2>
  <div class="panel-body">
    <slot />
  </div>
</section>
