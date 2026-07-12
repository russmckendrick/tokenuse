<script lang="ts">
  import { providerMark, toolMark } from './index';

  /** Provider id ("anthropic") or tool id ("claude-code") depending on kind. */
  export let id: string;
  export let kind: 'provider' | 'tool' = 'provider';
  export let size = 16;
  /** brand tints the mark with its provider accent; mono inherits text color. */
  export let variant: 'mono' | 'brand' = 'mono';

  $: mark = kind === 'provider' ? providerMark(id) : toolMark(id);
  $: brandColor = variant === 'brand' && kind === 'provider' ? `var(--provider-${id}, currentColor)` : null;
</script>

{#if mark}
  <span
    class="provider-icon"
    style={`width:${size}px;height:${size}px;${brandColor ? `color:${brandColor};` : ''}`}
    aria-hidden="true"
  >
    {@html mark}
  </span>
{/if}

<style>
  .provider-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
  }

  .provider-icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
