<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  interface BranchFile {
    path: string;
    status: string;
  }

  export let currentBranch: string = '';
  export let selectedFile: string | null = null;
  export let selectedBase: string | null = null;
  export let refreshTick: number = 0;

  const dispatch = createEventDispatcher();

  let branches: string[] = [];
  let baseBranch = '';
  let files: BranchFile[] = [];
  let loading = false;
  let error: string | null = null;

  async function loadBranches() {
    try {
      const raw = await invoke<{ name: string; remote: string | null }[]>('get_branch_list');
      branches = raw.filter(b => !b.remote).map(b => b.name);
      if (!baseBranch) {
        baseBranch =
          branches.find(b => b === 'main') ??
          branches.find(b => b === 'master') ??
          branches.find(b => b !== currentBranch) ??
          '';
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function loadFiles() {
    if (!baseBranch || !currentBranch || baseBranch === currentBranch) {
      files = [];
      return;
    }
    loading = true;
    error = null;
    try {
      files = await invoke<BranchFile[]>('get_branch_diff_files', {
        base: baseBranch,
        head: currentBranch,
      });
    } catch (e) {
      error = String(e);
      files = [];
    }
    loading = false;
  }

  $: baseBranch, currentBranch, loadFiles();
  $: refreshTick, loadFiles();

  onMount(loadBranches);

  function statusColor(status: string): string {
    if (status === 'added') return 'var(--accent-green)';
    if (status === 'deleted') return 'var(--accent-red)';
    return 'var(--accent-blue)';
  }

  function statusIcon(status: string): string {
    if (status === 'added') return 'A';
    if (status === 'deleted') return 'D';
    if (status === 'renamed') return 'R';
    return 'M';
  }

  function fileName(path: string): string {
    return path.split('/').pop() || path;
  }

  function fileDir(path: string): string {
    const parts = path.split('/');
    return parts.length > 1 ? parts.slice(0, -1).join('/') + '/' : '';
  }
</script>

<div class="branch-diff">
  <div class="branch-selector">
    <div class="branch-row">
      <span class="branch-label">Base</span>
      <select bind:value={baseBranch} class="branch-select">
        {#each branches as b}
          <option value={b}>{b}</option>
        {/each}
      </select>
    </div>
    <div class="branch-arrow">↓</div>
    <div class="branch-row">
      <span class="branch-label">Head</span>
      <span class="branch-current">{currentBranch || '—'}</span>
    </div>
  </div>

  <div class="file-list">
    {#if loading}
      <div class="status-msg">Loading…</div>
    {:else if error}
      <div class="status-msg error">{error}</div>
    {:else if baseBranch === currentBranch}
      <div class="status-msg">Base and head are the same branch.</div>
    {:else if files.length === 0}
      <div class="status-msg">No differences between branches.</div>
    {:else}
      <div class="section-header">
        <span>{files.length} changed {files.length === 1 ? 'file' : 'files'}</span>
      </div>
      {#each files as file}
        <button
          class="file-item"
          class:selected={selectedBase === baseBranch && selectedFile === file.path}
          on:click={() => dispatch('selectFile', { base: baseBranch, head: currentBranch, path: file.path })}
        >
          <span class="status" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
          <span class="path" title={file.path}>
            <span class="dir">{fileDir(file.path)}</span><span class="name">{fileName(file.path)}</span>
          </span>
        </button>
      {/each}
    {/if}
  </div>
</div>

<style>
  .branch-diff {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .branch-selector {
    padding: 10px 12px 8px;
    border-bottom: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .branch-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .branch-label {
    font-size: 11px;
    color: var(--text-muted);
    width: 32px;
    flex-shrink: 0;
  }

  .branch-select {
    flex: 1;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    color: var(--text-primary);
    font-size: 12px;
    font-family: var(--font-mono);
    padding: 3px 6px;
    cursor: pointer;
  }

  .branch-select:focus {
    outline: none;
    border-color: var(--accent-blue);
  }

  .branch-current {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--accent-blue);
  }

  .branch-arrow {
    font-size: 11px;
    color: var(--text-muted);
    padding-left: 36px;
  }

  .file-list {
    flex: 1;
    overflow-y: auto;
  }

  .section-header {
    padding: 6px 12px;
    font-size: 11px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border-color);
  }

  .status-msg {
    padding: 24px 16px;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
  }

  .status-msg.error {
    color: var(--accent-red);
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 12px;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
  }

  .file-item:hover {
    background: var(--bg-tertiary);
  }

  .file-item.selected {
    background: rgba(88, 166, 255, 0.15);
  }

  .status {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    width: 12px;
    text-align: center;
    flex-shrink: 0;
  }

  .path {
    font-family: var(--font-mono);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir {
    color: var(--text-muted);
  }

  .name {
    color: var(--text-primary);
  }
</style>
