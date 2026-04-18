<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  interface ThemeMeta {
    slug: string;
    name: string;
    swatch: string;
  }

  interface ThemeData {
    slug: string;
    name: string;
    css_vars: Record<string, string>;
  }

  let themes = $state<ThemeMeta[]>([]);
  let currentSlug = $state('cantaloupe');
  let open = $state(false);

  function applyTheme(data: ThemeData) {
    for (const [k, v] of Object.entries(data.css_vars)) {
      document.documentElement.style.setProperty(k, v);
    }
    currentSlug = data.slug;
    try {
      localStorage.setItem('captioner-theme-vars', JSON.stringify(data.css_vars));
    } catch (_) {}
  }

  onMount(async () => {
    try {
      const [themeList, current] = await Promise.all([
        invoke<ThemeMeta[]>('get_themes'),
        invoke<ThemeData>('get_current_theme'),
      ]);
      themes = themeList;
      applyTheme(current);
    } catch (e) {
      console.error('Theme load failed:', e);
    }
  });

  async function selectTheme(slug: string) {
    open = false;
    try {
      await invoke('set_theme', { slug });
      const data = await invoke<ThemeData>('get_current_theme');
      applyTheme(data);
    } catch (e) {
      console.error('Theme switch failed:', e);
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="fixed top-4 right-4 z-50">
  <button
    onclick={() => (open = !open)}
    class="flex h-8 w-8 items-center justify-center rounded-sm border border-border bg-background text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground transition-colors"
    aria-label="Choose theme"
    title="Choose theme"
  >
    <!-- Palette icon -->
    <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="13.5" cy="6.5" r=".5" fill="currentColor" />
      <circle cx="17.5" cy="10.5" r=".5" fill="currentColor" />
      <circle cx="8.5" cy="7.5" r=".5" fill="currentColor" />
      <circle cx="6.5" cy="12.5" r=".5" fill="currentColor" />
      <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" />
    </svg>
  </button>

  {#if open}
    <!-- click-outside backdrop -->
    <button
      class="fixed inset-0 z-40 cursor-default"
      onclick={() => (open = false)}
      aria-label="Close theme picker"
      tabindex="-1"
    ></button>

    <div class="absolute right-0 top-10 z-50 w-44 rounded-sm border border-border bg-background shadow-lg p-1">
      <p class="px-2 py-1 text-xs font-medium text-muted-foreground uppercase tracking-wide">Theme</p>
      {#each themes as theme (theme.slug)}
        <button
          onclick={() => selectTheme(theme.slug)}
          class="flex w-full items-center gap-2.5 rounded-sm px-2 py-1.5 text-sm text-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
        >
          <span
            class="h-3.5 w-3.5 shrink-0 rounded-full border border-border/50"
            style="background: {theme.swatch}"
          ></span>
          <span class="flex-1 text-left">{theme.name}</span>
          {#if theme.slug === currentSlug}
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
