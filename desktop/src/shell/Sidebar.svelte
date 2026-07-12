<script lang="ts">
  import {
    Boxes,
    ChartLine,
    FolderGit2,
    LayoutDashboard,
    PanelLeftClose,
    PanelLeftOpen,
    Settings,
    Wrench
  } from 'lucide-svelte';
  import ProviderIcon from '../icons/ProviderIcon.svelte';
  import type { CopyDeck, OptionItem, ToolId } from '../types';
  import type { Route, RouteToolId } from '../lib/router.svelte';

  export let copy: CopyDeck;
  export let route: Route;
  export let tools: OptionItem<ToolId>[];
  export let usageOrder: string[];
  export let collapsed: boolean;
  export let navigate: (route: Route) => void;
  export let toggleCollapsed: () => void;

  function isActive(page: Route['page']) {
    return route.page === page;
  }

  $: toolEntries = tools
    .filter((tool) => tool.value !== 'all')
    .sort((left, right) => {
      const leftRank = usageOrder.indexOf(left.label);
      const rightRank = usageOrder.indexOf(right.label);
      const leftOrder = leftRank === -1 ? Number.MAX_SAFE_INTEGER : leftRank;
      const rightOrder = rightRank === -1 ? Number.MAX_SAFE_INTEGER : rightRank;
      return leftOrder - rightOrder || left.label.localeCompare(right.label);
    });
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
      aria-current={isActive('overview') ? 'page' : undefined}
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
      aria-current={isActive('analytics') ? 'page' : undefined}
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
      aria-current={route.page === 'tools' && route.tool === undefined ? 'page' : undefined}
      title={copy.nav.tools}
      onclick={() => navigate({ page: 'tools' })}
    >
      <Wrench size={16} />
      {#if !collapsed}<span>{copy.nav.tools}</span>{/if}
    </button>

    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('models')}
      aria-current={isActive('models') ? 'page' : undefined}
      title={copy.nav.models}
      onclick={() => navigate({ page: 'models' })}
    >
      <Boxes size={16} />
      {#if !collapsed}<span>{copy.nav.models}</span>{/if}
    </button>

    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('projects')}
      aria-current={isActive('projects') ? 'page' : undefined}
      title={copy.nav.projects}
      onclick={() => navigate({ page: 'projects' })}
    >
      <FolderGit2 size={16} />
      {#if !collapsed}<span>{copy.nav.projects}</span>{/if}
    </button>

    {#each toolEntries as tool}
      <button
        type="button"
        class="sidebar-item"
        class:active={route.page === 'tools' && route.tool === tool.value}
        aria-current={route.page === 'tools' && route.tool === tool.value ? 'page' : undefined}
        title={tool.label}
        onclick={() => navigate({ page: 'tools', tool: tool.value as RouteToolId })}
      >
        <ProviderIcon id={tool.value} kind="tool" size={16} />
        {#if !collapsed}<span>{tool.label}</span>{/if}
      </button>
    {/each}
  </nav>

  <div class="sidebar-foot">
    <button
      type="button"
      class="sidebar-item"
      class:active={isActive('config')}
      aria-current={isActive('config') ? 'page' : undefined}
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
