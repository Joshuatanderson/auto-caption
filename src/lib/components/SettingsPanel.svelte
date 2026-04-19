<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { toast } from 'svelte-sonner';

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

  type CaptionPosition = 'top' | 'middle' | 'bottom';

  const POSITIONS: { value: CaptionPosition; label: string }[] = [
    { value: 'top',    label: 'Top' },
    { value: 'middle', label: 'Middle' },
    { value: 'bottom', label: 'Bottom' },
  ];

  interface CustomAssColors {
    primary_hex: string;
    accent_hex: string;
  }

  let themes = $state<ThemeMeta[]>([]);
  let currentSlug = $state('cantaloupe');
  let outputDir = $state<string | null>(null);
  let captionPosition = $state<CaptionPosition>('bottom');
  let panelOpen = $state(false);
  let customColors = $state<CustomAssColors>({
    primary_hex: '#FFFFFF',
    accent_hex: '#FFFF00',
  });

  function applyTheme(data: ThemeData) {
    for (const [k, v] of Object.entries(data.css_vars)) {
      document.documentElement.style.setProperty(k, v);
    }
    currentSlug = data.slug;
    try {
      localStorage.setItem('autocap-theme-vars', JSON.stringify(data.css_vars));
    } catch (_) {}
    if (data.slug === 'custom') {
      applyCustomOverrides(customColors.accent_hex);
    }
  }

  /** Relative luminance per WCAG; used to pick readable foreground on a tinted primary. */
  function hexLuminance(hex: string): number {
    const s = hex.replace('#', '');
    if (s.length !== 6) return 0;
    const [r, g, b] = [0, 2, 4].map(
      (i) => parseInt(s.slice(i, i + 2), 16) / 255,
    );
    const lin = (c: number) =>
      c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
  }

  /**
   * Derive UI accent vars from the user's Inner caption color so buttons,
   * rings, and selection states track the custom theme instead of staying
   * stuck on cantaloupe's green. Lightness tiers are produced via CSS
   * color-mix so we don't reimplement OKLCH in TS.
   */
  function applyCustomOverrides(innerHex: string) {
    const fg = hexLuminance(innerHex) > 0.55 ? 'oklch(0.17 0.01 264)' : 'oklch(1 0 0)';
    const pale = `color-mix(in oklch, ${innerHex} 18%, white)`;
    const paleFg = `color-mix(in oklch, ${innerHex} 55%, black)`;
    const ring = `color-mix(in oklch, ${innerHex} 65%, white)`;
    const overrides: Record<string, string> = {
      '--primary': innerHex,
      '--primary-foreground': fg,
      '--ring': ring,
      '--accent': pale,
      '--accent-foreground': paleFg,
      '--secondary': pale,
      '--secondary-foreground': paleFg,
      '--muted': pale,
      '--border': pale,
      '--input': pale,
      '--sidebar-primary': innerHex,
      '--sidebar-primary-foreground': fg,
      '--sidebar-accent': pale,
      '--sidebar-accent-foreground': paleFg,
      '--sidebar-border': pale,
      '--sidebar-ring': ring,
    };
    for (const [k, v] of Object.entries(overrides)) {
      document.documentElement.style.setProperty(k, v);
    }
  }

  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  onMount(async () => {
    try {
      const [themeList, current, dir, pos, custom] = await Promise.all([
        invoke<ThemeMeta[]>('get_themes'),
        invoke<ThemeData>('get_current_theme'),
        invoke<string | null>('get_output_dir'),
        invoke<CaptionPosition>('get_caption_position'),
        invoke<CustomAssColors>('get_custom_ass_colors'),
      ]);
      themes = themeList;
      outputDir = dir;
      captionPosition = pos;
      customColors = custom;
      // applyTheme must run after customColors is populated so that the
      // 'custom' path can derive its UI overrides from the actual accent hex.
      applyTheme(current);
    } catch (e) {
      console.error('Settings load failed:', e);
    }
  });

  async function updateCustomColor(which: 'outer' | 'inner', hex: string) {
    const next: CustomAssColors =
      which === 'outer'
        ? { ...customColors, primary_hex: hex }
        : { ...customColors, accent_hex: hex };
    customColors = next;
    if (currentSlug === 'custom' && which === 'inner') {
      applyCustomOverrides(next.accent_hex);
    }
    try {
      await invoke('set_custom_ass_colors', {
        primaryHex: next.primary_hex,
        accentHex: next.accent_hex,
      });
      themes = await invoke<ThemeMeta[]>('get_themes');
    } catch (e) {
      toast.error(
        `Failed to save custom color: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  async function selectTheme(slug: string) {
    try {
      await invoke('set_theme', { slug });
      const data = await invoke<ThemeData>('get_current_theme');
      applyTheme(data);
    } catch (e) {
      console.error('Theme switch failed:', e);
    }
  }

  async function pickOutputDir() {
    if (!isTauri) {
      toast.info('Folder picker requires the Tauri app — run `bun run tauri dev`');
      return;
    }
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        await invoke('set_output_dir', { path: selected });
        outputDir = selected;
        toast.success('Output directory updated');
      }
    } catch (e) {
      toast.error(`Folder picker failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  async function selectCaptionPosition(position: CaptionPosition) {
    const prev = captionPosition;
    captionPosition = position;
    try {
      await invoke('set_caption_position', { position });
    } catch (e) {
      captionPosition = prev;
      toast.error(`Failed to save position: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  async function clearOutputDir() {
    try {
      await invoke('set_output_dir', { path: null });
      outputDir = null;
      toast.success('Output directory cleared');
    } catch (e) {
      toast.error(`Failed to clear: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') panelOpen = false;
  }
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="fixed top-4 right-4 z-50">
  <button
    onclick={() => (panelOpen = !panelOpen)}
    class="flex h-8 w-8 items-center justify-center rounded-sm border border-border bg-background text-foreground shadow-sm hover:bg-accent hover:text-accent-foreground transition-colors"
    aria-label="Open settings"
    title="Settings"
  >
    <!-- Gear icon -->
    <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  </button>

  {#if panelOpen}
    <!-- click-outside backdrop -->
    <button
      class="fixed inset-0 z-40 cursor-default"
      onclick={() => (panelOpen = false)}
      aria-label="Close settings"
      tabindex="-1"
    ></button>

    <div class="absolute right-0 top-10 z-50 w-72 rounded-sm border border-border bg-background shadow-lg p-2 space-y-3">

      <!-- Output directory -->
      <div class="space-y-1.5">
        <p class="px-1 text-xs font-medium text-muted-foreground uppercase tracking-wide">Output directory</p>
        <p
          class="px-1 font-mono text-xs text-foreground truncate"
          title={outputDir ?? ''}
        >
          {outputDir ?? 'Not set — using video folder'}
        </p>
        <div class="flex gap-1">
          <button
            onclick={pickOutputDir}
            class="flex-1 rounded-sm border border-border bg-background px-2 py-1 text-xs text-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
          >
            Browse…
          </button>
          {#if outputDir}
            <button
              onclick={clearOutputDir}
              class="rounded-sm border border-border bg-background px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
            >
              Clear
            </button>
          {/if}
        </div>
      </div>

      <div class="border-t border-border"></div>

      <!-- Caption position -->
      <div class="space-y-1.5">
        <p class="px-1 text-xs font-medium text-muted-foreground uppercase tracking-wide">Caption position</p>
        <div class="grid grid-cols-3 gap-1">
          {#each POSITIONS as p (p.value)}
            {@const active = captionPosition === p.value}
            <button
              type="button"
              onclick={() => selectCaptionPosition(p.value)}
              aria-pressed={active}
              class="rounded-sm border px-2 py-1 text-xs transition-colors focus:outline-none focus:ring-2 focus:ring-ring"
              class:border-primary={active}
              class:bg-accent={active}
              class:text-accent-foreground={active}
              class:border-border={!active}
              class:text-foreground={!active}
              class:hover:bg-accent={!active}
              class:hover:text-accent-foreground={!active}
            >
              {p.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="border-t border-border"></div>

      <!-- Theme -->
      <div>
        <p class="px-1 pb-1 text-xs font-medium text-muted-foreground uppercase tracking-wide">Theme</p>
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
          {#if theme.slug === 'custom' && currentSlug === 'custom'}
            <div class="flex items-center gap-4 pl-7 pr-2 pb-1.5 pt-0.5">
              <label class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
                <input
                  type="color"
                  value={customColors.primary_hex}
                  onchange={(e) => updateCustomColor('outer', (e.currentTarget as HTMLInputElement).value)}
                  class="color-swatch"
                  aria-label="Outer caption color"
                />
                Outer
              </label>
              <label class="flex items-center gap-1.5 text-xs text-muted-foreground cursor-pointer">
                <input
                  type="color"
                  value={customColors.accent_hex}
                  onchange={(e) => updateCustomColor('inner', (e.currentTarget as HTMLInputElement).value)}
                  class="color-swatch"
                  aria-label="Inner caption color"
                />
                Inner
              </label>
            </div>
          {/if}
        {/each}
      </div>

    </div>
  {/if}
</div>
