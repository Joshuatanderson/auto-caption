<script lang="ts">
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { toast } from 'svelte-sonner';
  import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
  import CircleIcon from '@lucide/svelte/icons/circle';
  import CircleXIcon from '@lucide/svelte/icons/circle-x';
  import { Card, CardContent } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import SettingsPanel from '$lib/components/SettingsPanel.svelte';

  interface StageError {
    stage: string;
    message: string;
    stderr?: string;
  }

  type Stage = 'audio' | 'transcribe' | 'ass' | 'burn';
  type StepStatus = 'idle' | 'running' | 'done' | 'error';
  type OutputFormat = 'unchanged' | 'youtube-short' | 'linkedin-short' | 'square';

  type PipelineState =
    | { kind: 'idle' }
    | { kind: 'running'; stage: Stage }
    | { kind: 'done'; folder: string; files: string[] }
    | { kind: 'error'; stage: Stage; message: string; stderr?: string };

  interface PipelineResult { folder: string; files: string[]; }

  const FORMATS: { value: OutputFormat; label: string; hint: string }[] = [
    { value: 'unchanged',      label: 'Unchanged',    hint: 'Original dims' },
    { value: 'youtube-short',  label: 'YT Shorts',    hint: '9:16 · 1080×1920' },
    { value: 'linkedin-short', label: 'LI Shorts',    hint: '4:5 · 1080×1350' },
    { value: 'square',         label: 'Square',       hint: '1:1 · 1080×1080' },
  ];

  const STEPS: { key: Stage; label: string }[] = [
    { key: 'audio',      label: 'Extracting audio' },
    { key: 'transcribe', label: 'Transcribing' },
    { key: 'ass',        label: 'Generating caption files' },
    { key: 'burn',       label: 'Generating captioned videos' },
  ];
  const STAGE_ORDER: Stage[] = STEPS.map((s) => s.key);

  // --- State ---
  let droppedPath = $state<string | null>(null);
  let isDragging = $state(false);
  let selectedFormats = $state<Set<OutputFormat>>(new Set<OutputFormat>(['unchanged']));
  let pipeline = $state<PipelineState>({ kind: 'idle' });

  let formatsArray = $derived(
    FORMATS.filter((f) => selectedFormats.has(f.value)).map((f) => f.value),
  );
  let hasFormat = $derived(selectedFormats.size > 0);
  let canStart = $derived(
    droppedPath !== null && hasFormat && pipeline.kind !== 'running',
  );
  let lastError = $derived(
    pipeline.kind === 'error'
      ? pipeline.stderr
        ? `${pipeline.message}\n\n--- stderr ---\n${pipeline.stderr}`
        : pipeline.message
      : null,
  );

  // Changing the selected formats invalidates any completed/failed run so the
  // user doesn't see stale results for a different format set. Mid-run edits
  // shouldn't yank the pipeline out from under itself.
  function toggleFormat(value: OutputFormat) {
    const next = new Set(selectedFormats);
    if (next.has(value)) next.delete(value);
    else next.add(value);
    selectedFormats = next;
    if (pipeline.kind !== 'running') pipeline = { kind: 'idle' };
  }

  function stepStatus(key: Stage): StepStatus {
    if (pipeline.kind === 'idle') return 'idle';
    if (pipeline.kind === 'done') return 'done';
    const i = STAGE_ORDER.indexOf(key);
    const cur = STAGE_ORDER.indexOf(pipeline.stage);
    if (i < cur) return 'done';
    if (i > cur) return 'idle';
    return pipeline.kind === 'running' ? 'running' : 'error';
  }

  function basename(path: string): string {
    const slash = path.lastIndexOf('/');
    return slash === -1 ? path : path.slice(slash + 1);
  }

  function isMp4(path: string): boolean {
    return path.toLowerCase().endsWith('.mp4');
  }

  function parseError(err: unknown): StageError {
    if (typeof err === 'string') {
      try { return JSON.parse(err); } catch { /* fall through */ }
      return { stage: 'unknown', message: err };
    }
    if (err instanceof Error) return { stage: 'unknown', message: err.message };
    return { stage: 'unknown', message: String(err) };
  }

  function acceptPath(path: string) {
    if (isMp4(path)) {
      droppedPath = path;
      pipeline = { kind: 'idle' };
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
      toast.error(`File picker failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pickFile(); }
  }

  async function runPipeline() {
    if (!canStart || droppedPath === null) return;
    pipeline = { kind: 'running', stage: 'audio' };

    // Subscribe before invoke so we don't miss the first progress event on
    // tiny inputs where extract_audio returns near-instantly.
    const unlisten = await listen<{ stage: Stage }>('pipeline-progress', (event) => {
      if (pipeline.kind === 'running') {
        pipeline = { kind: 'running', stage: event.payload.stage };
      }
    });

    try {
      const res = await invoke<PipelineResult>('run_pipeline', {
        inputPath: droppedPath,
        formats: formatsArray,
      });
      pipeline = { kind: 'done', folder: res.folder, files: res.files };
      toast.success(
        `Done — ${res.files.length} captioned video${res.files.length === 1 ? '' : 's'} ready`,
      );
    } catch (err) {
      const parsed = parseError(err);
      const stage: Stage = pipeline.kind === 'running' ? pipeline.stage : 'audio';
      pipeline = {
        kind: 'error',
        stage,
        message: parsed.message,
        stderr: parsed.stderr,
      };
      toast.error(parsed.message);
    } finally {
      unlisten();
    }
  }

  $effect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') isDragging = true;
        else if (p.type === 'leave') isDragging = false;
        else if (p.type === 'drop') {
          isDragging = false;
          const path = p.paths[0];
          if (path) acceptPath(path);
        }
      })
      .then((fn) => { unlisten = fn; })
      .catch(() => {});
    return () => unlisten?.();
  });
</script>

<svelte:window ondragover={(e) => e.preventDefault()} ondrop={(e) => e.preventDefault()} />

<SettingsPanel />

<main class="flex min-h-screen items-center justify-center bg-background p-8">
  <Card class="w-full max-w-2xl">
    <CardContent class="p-8 space-y-6">

      <!-- Drop zone -->
      <div
        role="button"
        tabindex="0"
        onclick={pickFile}
        onkeydown={handleKey}
        class="flex cursor-pointer flex-col items-center justify-center gap-4 rounded-lg border-2 border-dashed p-12 text-center transition-colors hover:bg-accent/40 focus:outline-none focus:ring-2 focus:ring-ring"
        class:border-primary={isDragging}
        class:bg-accent={isDragging}
        class:border-border={!isDragging}
      >
        {#if droppedPath}
          <p class="text-sm text-muted-foreground">Loaded file</p>
          <code class="block w-full overflow-x-auto whitespace-nowrap rounded-md bg-muted px-4 py-2 font-mono text-sm text-foreground">
            {droppedPath}
          </code>
          <Button variant="ghost" onclick={(e) => { e.stopPropagation(); droppedPath = null; pipeline = { kind: 'idle' }; }}>
            Clear
          </Button>
        {:else if isDragging}
          <p class="text-lg font-medium text-foreground">Release to load</p>
        {:else}
          <p class="text-lg font-medium text-foreground">Drop an MP4 here</p>
          <p class="text-sm text-muted-foreground">or click to choose a file</p>
        {/if}
      </div>

      <!-- Output formats (multi-select) -->
      {#if droppedPath}
        <div class="space-y-2">
          <div class="flex items-baseline justify-between">
            <p class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Output formats</p>
            <p class="text-xs text-muted-foreground">{selectedFormats.size} selected</p>
          </div>
          <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
            {#each FORMATS as f (f.value)}
              {@const active = selectedFormats.has(f.value)}
              <button
                type="button"
                onclick={() => toggleFormat(f.value)}
                aria-pressed={active}
                class="flex flex-col items-start gap-0.5 rounded-md border px-3 py-2 text-left text-sm transition-colors focus:outline-none focus:ring-2 focus:ring-ring"
                class:border-primary={active}
                class:bg-accent={active}
                class:text-accent-foreground={active}
                class:border-border={!active}
                class:hover:bg-accent={!active}
                class:hover:text-accent-foreground={!active}
              >
                <span class="font-medium">{f.label}</span>
                <span class="text-xs text-muted-foreground">{f.hint}</span>
              </button>
            {/each}
          </div>
          {#if !hasFormat}
            <p class="text-xs text-destructive">Select at least one output format.</p>
          {/if}
        </div>
      {/if}

      <!-- Start / progress / done -->
      {#if droppedPath}
        {#if pipeline.kind === 'idle'}
          <Button onclick={runPipeline} disabled={!canStart} class="w-full">
            Generate
          </Button>
        {:else}
          <!-- Step list (running / error / done) -->
          <div class="space-y-3 rounded-lg border bg-muted/30 p-4">
            {#each STEPS as step (step.key)}
              {@const status = stepStatus(step.key)}
              <div class="flex items-center gap-3 text-sm">
                {#if status === 'done'}
                  <CircleCheckIcon class="size-5 shrink-0 text-primary" />
                {:else if status === 'running'}
                  <Spinner class="size-5 shrink-0 text-foreground" />
                {:else if status === 'error'}
                  <CircleXIcon class="size-5 shrink-0 text-destructive" />
                {:else}
                  <CircleIcon class="size-5 shrink-0 text-muted-foreground/50" />
                {/if}
                <span
                  class:text-muted-foreground={status === 'idle'}
                  class:font-medium={status === 'running' || status === 'done'}
                >
                  {step.label}
                </span>
              </div>
            {/each}
          </div>

          <!-- Success block -->
          {#if pipeline.kind === 'done'}
            {@const folder = pipeline.folder}
            <div class="flex flex-col items-center gap-3 rounded-lg border border-primary/30 bg-primary/5 p-6 text-center">
              <CircleCheckIcon class="size-10 text-primary" />
              <div>
                <p class="font-medium text-foreground">All done</p>
                <p class="mt-1 text-xs text-muted-foreground">
                  {pipeline.files.length} file{pipeline.files.length === 1 ? '' : 's'} in <span class="font-mono">{basename(folder)}/</span>
                </p>
              </div>
              <Button variant="outline" size="sm" onclick={() => revealItemInDir(folder)}>
                Show in Finder
              </Button>
            </div>
          {/if}

          <!-- Retry on error -->
          {#if pipeline.kind === 'error'}
            <Button onclick={runPipeline} disabled={!canStart} class="w-full">
              Retry
            </Button>
          {/if}
        {/if}

        <!-- Error block (stderr-safe) -->
        {#if lastError}
          <pre class="overflow-x-auto rounded-md bg-destructive/10 p-4 font-mono text-xs text-destructive whitespace-pre-wrap">{lastError}</pre>
        {/if}
      {/if}

    </CardContent>
  </Card>
</main>
