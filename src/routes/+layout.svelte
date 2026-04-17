<script lang="ts">
  import '../app.css';
  import type { Snippet } from 'svelte';
  import { Toaster } from '$lib/components/ui/sonner';

  interface Props {
    children?: Snippet;
  }

  let { children }: Props = $props();
</script>

{@render children?.()}
<Toaster />
