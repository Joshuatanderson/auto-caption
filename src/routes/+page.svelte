<script lang="ts">
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { open } from '@tauri-apps/plugin-dialog';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';

  let droppedPath = $state<string | null>(null);
  let isDragging = $state(false);

  function basename(path: string): string {
    const slash = path.lastIndexOf('/');
    return slash === -1 ? path : path.slice(slash + 1);
  }

  function isMp4(path: string): boolean {
    return path.toLowerCase().endsWith('.mp4');
  }

  function acceptPath(path: string) {
    if (isMp4(path)) {
      droppedPath = path;
      toast.success(`Loaded: ${basename(path)}`);
    } else {
      toast.error('Only .mp4 files are supported');
    }
  }

  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  async function pickFile() {
    if (!isTauri) {
      toast.info('Click-to-pick requires the Tauri app — run `bun run tauri dev`');
      return;
    }
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Video', extensions: ['mp4'] }],
      });
      if (typeof selected === 'string') acceptPath(selected);
    } catch (err) {
      console.error('file picker failed', err);
      toast.error(`File picker failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      pickFile();
    }
  }

  $effect(() => {
    let unlisten: (() => void) | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') {
          isDragging = true;
        } else if (p.type === 'leave') {
          isDragging = false;
        } else if (p.type === 'drop') {
          isDragging = false;
          const path = p.paths[0];
          if (path) acceptPath(path);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        // Running outside Tauri (e.g. `bun run dev` in a browser) — no drag/drop.
      });

    return () => unlisten?.();
  });
</script>

<svelte:window
  ondragover={(e) => e.preventDefault()}
  ondrop={(e) => e.preventDefault()}
/>

<main class="flex min-h-screen items-center justify-center bg-background p-8">
  <Card class="w-full max-w-2xl">
    <CardContent class="p-8">
      <div
        role="button"
        tabindex="0"
        onclick={pickFile}
        onkeydown={handleKey}
        class="flex cursor-pointer flex-col items-center justify-center gap-4 rounded-lg border-2 border-dashed p-16 text-center transition-colors hover:bg-accent/40 focus:outline-none focus:ring-2 focus:ring-ring"
        class:border-primary={isDragging}
        class:bg-accent={isDragging}
        class:border-border={!isDragging}
      >
        {#if droppedPath}
          <p class="text-sm text-muted-foreground">Loaded file</p>
          <code
            class="block w-full overflow-x-auto whitespace-nowrap rounded-md bg-muted px-4 py-2 font-mono text-sm text-foreground"
          >
            {droppedPath}
          </code>
          <Button
            variant="ghost"
            onclick={(e) => {
              e.stopPropagation();
              droppedPath = null;
            }}
          >
            Clear
          </Button>
        {:else if isDragging}
          <p class="text-lg font-medium text-foreground">Release to load</p>
        {:else}
          <p class="text-lg font-medium text-foreground">Drop an MP4 here</p>
          <p class="text-sm text-muted-foreground">or click to choose a file</p>
        {/if}
      </div>
    </CardContent>
  </Card>
</main>
