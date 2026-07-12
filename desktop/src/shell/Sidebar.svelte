<script lang="ts">
  import {
    ChartLine,
    LayoutDashboard,
    PanelLeftClose,
    PanelLeftOpen,
    Settings,
    Terminal,
    Wrench
  } from 'lucide-svelte';
  import type { CopyDeck, OptionItem, ToolId } from '../types';
  import type { Route, RouteToolId } from '../lib/router.svelte';

  export let copy: CopyDeck;
  export let route: Route;
  export let tools: OptionItem<ToolId>[];
  export let collapsed: boolean;
  export let navigate: (route: Route) => void;
  export let toggleCollapsed: () => void;

  $: toolEntries = tools.filter((tool) => tool.value !== 'all');

  function isActive(page: Route['page'], tool?: RouteToolId) {
    if (route.page !== page) return false;
    return tool === undefined || route.tool === tool;
  }
</script>

<aside class="sidebar" class:collapsed aria-label={copy.desktop.nav_aria}>
  <div class="sidebar-brand">
    <svg class="brand-bars" viewBox="0 0 440 560" aria-hidden="true">
      <defs>
        <linearGradient id="sidebar-bar-gradient" x1="0" y1="0" x2="0" y2="560" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stop-color="#ffc06a" />
          <stop offset="45%" stop-color="#ff9a4d" />
          <stop offset="100%" stop-color="#f26a3d" />
        </linearGradient>
      </defs>
      <rect x="0" y="280" width="80" height="280" rx="16" fill="url(#sidebar-bar-gradient)" />
      <rect x="120" y="160" width="80" height="400" rx="16" fill="url(#sidebar-bar-gradient)" />
      <rect x="240" y="0" width="80" height="560" rx="16" fill="url(#sidebar-bar-gradient)" />
      <rect x="360" y="120" width="80" height="440" rx="16" fill="url(#sidebar-bar-gradient)" />
    </svg>
    {#if !collapsed}
      <span class="sidebar-title">{copy.brand.name}</span>
    {/if}
  </div>

  <nav class="sidebar-nav" aria-label={copy.desktop.nav_aria}>
    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('overview')}
      title={copy.nav.overview}
      onclick={() => navigate({ page: 'overview' })}
    >
      <LayoutDashboard size={16} />
      {#if !collapsed}<span>{copy.nav.overview}</span>{/if}
    </button>

    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('analytics')}
      title={copy.nav.analytics}
      onclick={() => navigate({ page: 'analytics' })}
    >
      <ChartLine size={16} />
      {#if !collapsed}<span>{copy.nav.analytics}</span>{/if}
    </button>

    <button
      type="button"
      class="sidebar-item"
      class:active={route.page === 'tools' && route.tool === undefined}
      title={copy.nav.tools}
      onclick={() => navigate({ page: 'tools' })}
    >
      <Wrench size={16} />
      {#if !collapsed}<span>{copy.nav.tools}</span>{/if}
    </button>

    {#if !collapsed}
      <div class="sidebar-children">
        {#each toolEntries as tool}
          <button
            type="button"
            class="sidebar-item child"
            class:active={isActive('tools', tool.value as RouteToolId)}
            title={tool.label}
            onclick={() => navigate({ page: 'tools', tool: tool.value as RouteToolId })}
          >
            <Terminal size={14} />
            <span>{tool.label}</span>
          </button>
        {/each}
      </div>
    {/if}
  </nav>

  <div class="sidebar-foot">
    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('config')}
      title={copy.nav.config}
      onclick={() => navigate({ page: 'config' })}
    >
      <Settings size={16} />
      {#if !collapsed}<span>{copy.nav.config}</span>{/if}
    </button>
    <button
      type="button"
      class="sidebar-item"
      title={collapsed ? copy.desktop.expand_sidebar : copy.desktop.collapse_sidebar}
      onclick={toggleCollapsed}
    >
      {#if collapsed}
        <PanelLeftOpen size={16} />
      {:else}
        <PanelLeftClose size={16} />
        <span>{copy.desktop.collapse_sidebar}</span>
      {/if}
    </button>
  </div>
</aside>
