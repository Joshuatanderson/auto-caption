<script lang="ts">
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { toast } from 'svelte-sonner';
  import { Card, CardContent } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Progress } from '$lib/components/ui/progress';

  // --- Whisper types (must match src-tauri/src/pipeline/types.rs) ---
  interface WhisperToken {
    text: string;
    timestamps: { from: string; to: string };
    offsets: { from: number; to: number };
    id: number;
    p: number;
  }
  interface WhisperSegment {
    timestamps: { from: string; to: string };
    offsets: { from: number; to: number };
    text: string;
    tokens: WhisperToken[];
  }
  interface WhisperOutput {
    transcription: WhisperSegment[];
  }
  interface StageError {
    stage: string;
    message: string;
    stderr?: string;
  }

  type StageStatus = 'idle' | 'running' | 'done' | 'error';

  // --- Input ---
  let droppedPath = $state<string | null>(null);
  let isDragging = $state(false);

  // --- Pipeline stage state ---
  let audioStatus = $state<StageStatus>('idle');
  let audioPath = $state<string | null>(null);

  let transcribeStatus = $state<StageStatus>('idle');
  let transcript = $state<WhisperOutput | null>(null);
  let transcriptSnippet = $state('');

  let assStatus = $state<StageStatus>('idle');
  let assPath = $state<string | null>(null);

  let burnStatus = $state<StageStatus>('idle');
  let burnPath = $state<string | null>(null);

  let lastError = $state<string | null>(null);

  // --- Derived enable conditions ---
  let canExtract = $derived(droppedPath !== null && audioStatus !== 'running');
  let canTranscribe = $derived(audioPath !== null && transcribeStatus !== 'running');
  let canGenerateAss = $derived(transcript !== null && assStatus !== 'running');
  let canBurn = $derived(assPath !== null && burnStatus !== 'running');

  // --- Helpers ---
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

  function showError(e: StageError) {
    lastError = e.stderr ? `${e.message}\n\n--- stderr ---\n${e.stderr}` : e.message;
    toast.error(e.message);
  }

  function acceptPath(path: string) {
    if (isMp4(path)) {
      droppedPath = path;
      // Reset downstream state when a new file is loaded
      audioStatus = 'idle'; audioPath = null;
      transcribeStatus = 'idle'; transcript = null; transcriptSnippet = '';
      assStatus = 'idle'; assPath = null;
      burnStatus = 'idle'; burnPath = null;
      lastError = null;
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

  // --- Stage handlers ---
  async function runExtract() {
    audioStatus = 'running'; lastError = null;
    try {
      audioPath = await invoke<string>('extract_audio', { inputPath: droppedPath });
      audioStatus = 'done';
      toast.success('Audio extracted');
    } catch (err) {
      audioStatus = 'error';
      showError(parseError(err));
    }
  }

  async function runTranscribe() {
    transcribeStatus = 'running'; lastError = null;
    try {
      transcript = await invoke<WhisperOutput>('transcribe', { wavPath: audioPath });
      const fullText = transcript.transcription.map((s) => s.text).join('').trim();
      transcriptSnippet = fullText.length > 160 ? fullText.slice(0, 160) + '…' : fullText;
      transcribeStatus = 'done';
      toast.success('Transcribed');
    } catch (err) {
      transcribeStatus = 'error';
      showError(parseError(err));
    }
  }

  async function runGenerateAss() {
    assStatus = 'running'; lastError = null;
    try {
      assPath = await invoke<string>('generate_ass', {
        inputPath: droppedPath,
        transcript,
      });
      assStatus = 'done';
      toast.success('Captions generated');
    } catch (err) {
      assStatus = 'error';
      showError(parseError(err));
    }
  }

  async function runBurn() {
    burnStatus = 'running'; lastError = null;
    try {
      burnPath = await invoke<string>('burn_captions', {
        inputPath: droppedPath,
        assPath,
      });
      burnStatus = 'done';
      toast.success('Done! Captioned video ready.');
    } catch (err) {
      burnStatus = 'error';
      showError(parseError(err));
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
          <Button variant="ghost" onclick={(e) => { e.stopPropagation(); acceptPath(''); droppedPath = null; }}>
            Clear
          </Button>
        {:else if isDragging}
          <p class="text-lg font-medium text-foreground">Release to load</p>
        {:else}
          <p class="text-lg font-medium text-foreground">Drop an MP4 here</p>
          <p class="text-sm text-muted-foreground">or click to choose a file</p>
        {/if}
      </div>

      <!-- Pipeline stages -->
      {#if droppedPath}
        <div class="space-y-3">

          <!-- Extract Audio -->
          <div class="flex items-center gap-3">
            <div class="w-36 shrink-0">
              <Button onclick={runExtract} disabled={!canExtract} class="w-full">
                {audioStatus === 'running' ? 'Extracting…' : 'Extract'}
              </Button>
            </div>
            <div class="min-w-0 flex-1 text-sm">
              {#if audioStatus === 'running'}
                <Progress />
              {:else if audioStatus === 'done' && audioPath}
                <span class="font-mono text-muted-foreground truncate block">{basename(audioPath)}</span>
              {:else if audioStatus === 'error'}
                <span class="text-destructive">Failed</span>
              {:else}
                <span class="text-muted-foreground">Extract 16kHz mono WAV</span>
              {/if}
            </div>
          </div>

          <!-- Transcribe -->
          <div class="flex items-center gap-3">
            <div class="w-36 shrink-0">
              <Button onclick={runTranscribe} disabled={!canTranscribe} class="w-full">
                {transcribeStatus === 'running' ? 'Transcribing…' : 'Transcribe'}
              </Button>
            </div>
            <div class="min-w-0 flex-1 text-sm">
              {#if transcribeStatus === 'running'}
                <Progress />
              {:else if transcribeStatus === 'done'}
                <span class="text-muted-foreground italic">{transcriptSnippet}</span>
              {:else if transcribeStatus === 'error'}
                <span class="text-destructive">Failed</span>
              {:else}
                <span class="text-muted-foreground">Run whisper-cli</span>
              {/if}
            </div>
          </div>

          <!-- Generate .ass -->
          <div class="flex items-center gap-3">
            <div class="w-36 shrink-0">
              <Button onclick={runGenerateAss} disabled={!canGenerateAss} class="w-full">
                {assStatus === 'running' ? 'Generating…' : 'Generate'}
              </Button>
            </div>
            <div class="min-w-0 flex-1 text-sm">
              {#if assStatus === 'running'}
                <Progress />
              {:else if assStatus === 'done' && assPath}
                <span class="font-mono text-muted-foreground truncate block">{basename(assPath)}</span>
              {:else if assStatus === 'error'}
                <span class="text-destructive">Failed</span>
              {:else}
                <span class="text-muted-foreground">Build .ass subtitle file</span>
              {/if}
            </div>
          </div>

          <!-- Burn Captions -->
          <div class="flex items-center gap-3">
            <div class="w-36 shrink-0">
              <Button onclick={runBurn} disabled={!canBurn} class="w-full">
                {burnStatus === 'running' ? 'Burning…' : 'Burn'}
              </Button>
            </div>
            <div class="min-w-0 flex-1 text-sm">
              {#if burnStatus === 'running'}
                <Progress />
              {:else if burnStatus === 'done' && burnPath}
                <div class="flex items-center gap-2">
                  <span class="font-mono text-muted-foreground truncate">{basename(burnPath)}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onclick={() => revealItemInDir(burnPath!)}
                    class="shrink-0"
                  >
                    Show in Finder
                  </Button>
                </div>
              {:else if burnStatus === 'error'}
                <span class="text-destructive">Failed</span>
              {:else}
                <span class="text-muted-foreground">Burn captions into MP4</span>
              {/if}
            </div>
          </div>

        </div>

        <!-- Error block (stderr-safe) -->
        {#if lastError}
          <pre class="overflow-x-auto rounded-md bg-destructive/10 p-4 font-mono text-xs text-destructive whitespace-pre-wrap">{lastError}</pre>
        {/if}
      {/if}

    </CardContent>
  </Card>
</main>
