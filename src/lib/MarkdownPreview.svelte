<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { marked } from 'marked';

  export let filePath: string;

  let html = '';
  let loading = true;
  let error: string | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      const content = await invoke<string>('read_file', { path: filePath });
      html = await marked(content);
    } catch (e) {
      error = String(e);
    }
    loading = false;
  }

  $: filePath, load();
</script>

<div class="markdown-preview">
  {#if loading}
    <div class="status">Loading…</div>
  {:else if error}
    <div class="status error">{error}</div>
  {:else}
    <div class="markdown-body">{@html html}</div>
  {/if}
</div>

<style>
  .markdown-preview {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    padding: 24px 32px;
    background: var(--bg-primary);
  }

  .status {
    padding: 32px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }

  .status.error {
    color: var(--accent-red);
  }

  .markdown-body {
    max-width: 800px;
    margin: 0 auto;
    color: var(--text-primary);
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
    font-size: 14px;
    line-height: 1.6;
  }

  .markdown-body :global(h1),
  .markdown-body :global(h2),
  .markdown-body :global(h3),
  .markdown-body :global(h4),
  .markdown-body :global(h5),
  .markdown-body :global(h6) {
    margin-top: 24px;
    margin-bottom: 12px;
    font-weight: 600;
    line-height: 1.25;
    color: var(--text-primary);
  }

  .markdown-body :global(h1) { font-size: 2em; border-bottom: 1px solid var(--border-color); padding-bottom: 8px; }
  .markdown-body :global(h2) { font-size: 1.5em; border-bottom: 1px solid var(--border-color); padding-bottom: 6px; }
  .markdown-body :global(h3) { font-size: 1.25em; }

  .markdown-body :global(p) {
    margin-top: 0;
    margin-bottom: 16px;
  }

  .markdown-body :global(a) {
    color: var(--accent-blue);
    text-decoration: none;
  }

  .markdown-body :global(a:hover) {
    text-decoration: underline;
  }

  .markdown-body :global(code) {
    font-family: var(--font-mono);
    font-size: 85%;
    background: var(--bg-tertiary);
    padding: 2px 5px;
    border-radius: 4px;
    color: var(--accent-red);
  }

  .markdown-body :global(pre) {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 16px;
    overflow-x: auto;
    margin-bottom: 16px;
  }

  .markdown-body :global(pre code) {
    background: transparent;
    padding: 0;
    color: var(--text-primary);
    font-size: 13px;
  }

  .markdown-body :global(blockquote) {
    margin: 0 0 16px 0;
    padding: 0 16px;
    border-left: 4px solid var(--border-color);
    color: var(--text-secondary);
  }

  .markdown-body :global(ul),
  .markdown-body :global(ol) {
    margin-bottom: 16px;
    padding-left: 24px;
  }

  .markdown-body :global(li) {
    margin-bottom: 4px;
  }

  .markdown-body :global(table) {
    border-collapse: collapse;
    width: 100%;
    margin-bottom: 16px;
  }

  .markdown-body :global(th),
  .markdown-body :global(td) {
    border: 1px solid var(--border-color);
    padding: 6px 12px;
    text-align: left;
  }

  .markdown-body :global(th) {
    background: var(--bg-secondary);
    font-weight: 600;
  }

  .markdown-body :global(tr:nth-child(even)) {
    background: var(--bg-secondary);
  }

  .markdown-body :global(hr) {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 24px 0;
  }

  .markdown-body :global(img) {
    max-width: 100%;
  }
</style>
