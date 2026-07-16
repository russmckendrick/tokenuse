<script lang="ts">
  export let score = 0;
  export let label = '';
  export let sublabel = '';
  export let size = 116;
  export let ariaLabel = '';

  $: clamped = Math.max(0, Math.min(100, Number.isFinite(score) ? score : 0));
  // Grade-tier tones: green is earned by A-tier only (≥90), B/C read as
  // needs-attention, D/F as trouble.
  $: tone = clamped >= 90 ? 'good' : clamped >= 70 ? 'warn' : 'bad';
</script>

<span
  class={`radial-gauge ${tone}`}
  role="img"
  aria-label={`${ariaLabel}: ${Math.round(clamped)}/100`}
  style={`width:${size}px;height:${size}px`}
>
  <svg viewBox="0 0 44 44" aria-hidden="true">
    <circle class="gauge-track" cx="22" cy="22" r="19" pathLength="100"></circle>
    <circle
      class="gauge-progress"
      cx="22"
      cy="22"
      r="19"
      pathLength="100"
      stroke-dasharray={`${clamped} ${100 - clamped}`}
    ></circle>
  </svg>
  <span class="gauge-center">
    <strong class="mono gauge-grade">{label}</strong>
    {#if sublabel}
      <span class="mono gauge-score">{sublabel}</span>
    {/if}
  </span>
</span>

<style>
  .radial-gauge {
    position: relative;
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
    stroke-width: 3;
  }

  .gauge-track {
    stroke: var(--color-bar-empty);
  }

  .gauge-progress {
    stroke-linecap: round;
    transition: stroke-dasharray var(--motion-slow) var(--ease-standard);
  }

  .radial-gauge.good .gauge-progress {
    stroke: var(--color-tertiary);
  }

  .radial-gauge.warn .gauge-progress {
    stroke: var(--color-warning);
  }

  .radial-gauge.bad .gauge-progress {
    stroke: var(--color-error);
  }

  .gauge-center {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 2px;
    text-align: center;
  }

  .gauge-grade {
    font-size: var(--text-display-xl);
    font-weight: 700;
    line-height: 1;
  }

  .radial-gauge.good .gauge-grade {
    color: var(--color-tertiary);
  }

  .radial-gauge.warn .gauge-grade {
    color: var(--color-warning);
  }

  .radial-gauge.bad .gauge-grade {
    color: var(--color-error);
  }

  .gauge-score {
    font-size: var(--text-label);
    color: var(--color-muted);
  }

  @media (prefers-reduced-motion: reduce) {
    .gauge-progress {
      transition: none;
    }
  }
</style>
