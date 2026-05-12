<script lang="ts">
  import { Database, FileText, Play } from 'lucide-svelte';
  import type { AdviceDataScopeId } from '../../types';
  import type { InsightsCopy } from './model';

  export let copy: InsightsCopy;
  export let adviceRunning: boolean;
  export let selectedAdviceScope: AdviceDataScopeId;
  export let setAdviceScope: (scope: AdviceDataScopeId) => void;
  export let runSelectedAdvice: () => void;
  export let controlId = 'header';

  let scopeOptions: Array<{
    id: AdviceDataScopeId;
    label: string;
    detail: string;
  }> = [];

  $: scopeOptions = [
    {
      id: 'redacted',
      label: copy.advice_scope_redacted,
      detail: copy.advice_scope_redacted_detail
    },
    {
      id: 'prompt_snippets',
      label: copy.advice_scope_snippets,
      detail: copy.advice_scope_snippets_detail
    }
  ];
</script>

<div class="advice-generator">
  <span class="advice-generator-label">{copy.advice_scope_title}</span>
  <div class="advice-generator-row">
    <div class="advice-scope-options" role="radiogroup" aria-label={copy.advice_scope_title}>
      {#each scopeOptions as option}
        <label
          class="advice-scope-option"
          class:active={selectedAdviceScope === option.id}
          class:disabled={adviceRunning}
          title={option.detail}
          aria-label={`${option.label}. ${option.detail}`}
        >
          <input
            type="radio"
            name={`advice-scope-${controlId}`}
            value={option.id}
            checked={selectedAdviceScope === option.id}
            disabled={adviceRunning}
            onchange={() => setAdviceScope(option.id)}
          />
          {#if option.id === 'redacted'}
            <Database size={14} />
          {:else}
            <FileText size={14} />
          {/if}
          <span>
            <strong>{option.label}</strong>
          </span>
        </label>
      {/each}
    </div>

    <button
      class="primary-action run-advice-button"
      type="button"
      disabled={adviceRunning}
      aria-busy={adviceRunning}
      onclick={runSelectedAdvice}
    >
      {#if adviceRunning}
        {copy.advice_scope_running}
      {:else}
        <Play size={13} /> {copy.advice_generate_button}
      {/if}
    </button>
  </div>
</div>
