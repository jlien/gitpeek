<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  export interface CommitInfo {
    hash: string;
    short_hash: string;
    message: string;
    author: string;
    time: number;
  }

  interface CommitFile {
    path: string;
    status: string;
  }

  export let commits: CommitInfo[] = [];
  export let selectedHash: string | null = null;
  export let selectedFile: string | null = null;

  const dispatch = createEventDispatcher();

  let expandedHash: string | null = null;
  let filesByHash = new Map<string, CommitFile[]>();
  let loadingHash: string | null = null;

  async function toggleCommit(hash: string) {
    if (expandedHash === hash) {
      expandedHash = null;
      return;
    }
    expandedHash = hash;
    if (!filesByHash.has(hash)) {
      loadingHash = hash;
      try {
        const files = await invoke<CommitFile[]>('get_commit_files', { hash });
        filesByHash = new Map(filesByHash).set(hash, files);
      } catch (e) {
        console.error(e);
      }
      loadingHash = null;
    }
  }

  function relativeTime(ts: number): string {
    const diff = Date.now() / 1000 - ts;
    if (diff < 60) return 'just now';
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
    if (diff < 86400 * 365) return `${Math.floor(diff / 86400 / 30)}mo ago`;
    return `${Math.floor(diff / 86400 / 365)}y ago`;
  }

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

<div class="commit-log">
  {#if commits.length === 0}
    <div class="empty">No commits found</div>
  {:else}
    {#each commits as commit}
      {@const isExpanded = expandedHash === commit.hash}
      <div class="commit" class:expanded={isExpanded}>
        <button
          class="commit-header"
          class:active={selectedHash === commit.hash}
          on:click={() => toggleCommit(commit.hash)}
        >
          <div class="commit-top">
            <span class="short-hash">{commit.short_hash}</span>
            <span class="chevron" class:open={isExpanded}>›</span>
          </div>
          <div class="commit-message">{commit.message}</div>
          <div class="commit-meta">{commit.author} · {relativeTime(commit.time)}</div>
        </button>

        {#if isExpanded}
          <div class="file-list">
            {#if loadingHash === commit.hash}
              <div class="loading">Loading…</div>
            {:else}
              {#each filesByHash.get(commit.hash) ?? [] as file}
                <button
                  class="file-item"
                  class:selected={selectedHash === commit.hash && selectedFile === file.path}
                  on:click={() => dispatch('selectFile', { hash: commit.hash, path: file.path })}
                >
                  <span class="status" style="color: {statusColor(file.status)}">{statusIcon(file.status)}</span>
                  <span class="path" title={file.path}>
                    <span class="dir">{fileDir(file.path)}</span><span class="name">{fileName(file.path)}</span>
                  </span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .commit-log {
    padding: 4px 0;
  }

  .commit {
    border-bottom: 1px solid var(--border-color);
  }

  .commit-header {
    width: 100%;
    padding: 8px 12px;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
  }

  .commit-header:hover {
    background: var(--bg-tertiary);
  }

  .commit-header.active {
    background: rgba(88, 166, 255, 0.1);
  }

  .commit-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2px;
  }

  .short-hash {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent-blue);
  }

  .chevron {
    font-size: 14px;
    color: var(--text-muted);
    transform: rotate(0deg);
    transition: transform 0.15s;
    display: inline-block;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .commit-message {
    font-size: 12px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 2px;
  }

  .commit-meta {
    font-size: 11px;
    color: var(--text-muted);
  }

  .file-list {
    background: var(--bg-primary);
    border-top: 1px solid var(--border-color);
  }

  .loading {
    padding: 8px 16px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 12px 5px 20px;
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

  .empty {
    padding: 32px 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
