<script lang="ts">
  import ProviderIcon from '../icons/ProviderIcon.svelte';

  export let id: string;
  export let used = 0;
  export let ariaLabel = '';
  export let usedSuffix = 'used';

  function providerForTool(toolId: string): string {
    switch (toolId) {
      case 'claude-code':
        return 'anthropic';
      case 'codex':
        return 'openai';
      case 'copilot':
        return 'github';
      case 'cursor':
        return 'cursor';
      case 'gemini':
        return 'google';
      default:
        return 'other';
    }
  }

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(used) ? used : 0));
  $: tone = clamped >= 88 ? 'hot' : clamped >= 60 ? 'warm' : 'cool';
  $: provider = providerForTool(id);
</script>

<span
  class="tool-gauge"
  role="img"
  aria-label={`${ariaLabel}: ${Math.round(clamped)}% ${usedSuffix}`}
  style={`--tool-accent: var(--provider-${provider})`}
>
  <svg viewBox="0 0 44 44" aria-hidden="true">
    <circle class="gauge-track" cx="22" cy="22" r="18" pathLength="100"></circle>
    <circle
      class={`gauge-progress ${tone}`}
      cx="22"
      cy="22"
      r="18"
      pathLength="100"
      stroke-dasharray={`${clamped} ${100 - clamped}`}
    ></circle>
  </svg>
  <span class="tool-gauge-icon"><ProviderIcon {id} kind="tool" size={23} /></span>
</span>

<style>
  .tool-gauge {
    position: relative;
    width: 52px;
    height: 52px;
    display: grid;
    place-items: center;
    flex: 0 0 auto;
  }

  svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
    stroke-width: 4;
  }

  .gauge-track {
    stroke: var(--color-bar-empty);
  }

  .gauge-progress {
    stroke-linecap: round;
    transition: stroke-dasharray var(--motion-slow) var(--ease-standard);
  }

  .gauge-progress.cool {
    stroke: var(--color-cyan);
  }

  .gauge-progress.warm {
    stroke: var(--color-warning);
  }

  .gauge-progress.hot {
    stroke: var(--color-error);
  }

  .tool-gauge-icon {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--tool-accent);
  }
</style>
