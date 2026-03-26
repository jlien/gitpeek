<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  export let filePath: string;

  let src = '';
  let loading = true;
  let error: string | null = null;

  const MIME: Record<string, string> = {
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    gif: 'image/gif',
    webp: 'image/webp',
    svg: 'image/svg+xml',
    bmp: 'image/bmp',
    ico: 'image/x-icon',
    tiff: 'image/tiff',
    tif: 'image/tiff',
    avif: 'image/avif',
  };

  function mimeType(path: string): string {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    return MIME[ext] ?? 'application/octet-stream';
  }

  async function load() {
    loading = true;
    error = null;
    src = '';
    try {
      const b64 = await invoke<string>('read_file_base64', { path: filePath });
      src = `data:${mimeType(filePath)};base64,${b64}`;
    } catch (e) {
      error = String(e);
    }
    loading = false;
  }

  $: filePath, load();
</script>

<div class="image-preview">
  {#if loading}
    <div class="status">Loading…</div>
  {:else if error}
    <div class="status error">{error}</div>
  {:else}
    <div class="image-wrap">
      <img {src} alt={filePath} />
    </div>
  {/if}
</div>

<style>
  .image-preview {
    flex: 1;
    overflow: auto;
    background: var(--bg-primary);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 32px;
  }

  .status {
    margin: auto;
    color: var(--text-muted);
    font-size: 13px;
  }

  .status.error {
    color: var(--accent-red);
  }

  .image-wrap {
    max-width: 100%;
  }

  img {
    display: block;
    max-width: 100%;
    border-radius: 4px;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.4);
  }
</style>
