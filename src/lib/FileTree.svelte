<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  interface FileChange {
    path: string;
    status: 'added' | 'modified' | 'deleted' | 'renamed';
    staged: boolean;
    additions: number;
    deletions: number;
  }

  export let files: FileChange[] = [];
  export let selectedFile: string | null = null;

  const dispatch = createEventDispatcher();

  $: stagedFiles = files.filter(f => f.staged);
  $: unstagedFiles = files.filter(f => !f.staged);

  function getStatusIcon(status: string): string {
    switch (status) {
      case 'added': return 'A';
      case 'modified': return 'M';
      case 'deleted': return 'D';
      case 'renamed': return 'R';
      default: return '?';
    }
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case 'added': return 'var(--accent-green)';
      case 'deleted': return 'var(--accent-red)';
      default: return 'var(--accent-blue)';
    }
  }

  function getFileName(path: string): string {
    return path.split('/').pop() || path;
  }

  function getDirectory(path: string): string {
    const parts = path.split('/');
    if (parts.length > 1) {
      return parts.slice(0, -1).join('/') + '/';
    }
    return '';
  }
</script>

<div class="file-tree">
  {#if stagedFiles.length > 0}
    <div class="section">
      <div class="section-header">
        <span class="section-title">Staged Changes</span>
        <span class="count">{stagedFiles.length}</span>
      </div>
      {#each stagedFiles as file}
        <div
          class="file-item"
          class:selected={selectedFile === file.path}
          role="button"
          tabindex="0"
          on:click={() => dispatch('select', file.path)}
          on:keydown={(e) => e.key === 'Enter' && dispatch('select', file.path)}
        >
          <span class="status" style="color: {getStatusColor(file.status)}">
            {getStatusIcon(file.status)}
          </span>
          <span class="path">
            <span class="dir">{getDirectory(file.path)}</span>
            <span class="name">{getFileName(file.path)}</span>
          </span>
          <div class="actions">
            <button 
              class="action-btn" 
              title="Unstage"
              on:click|stopPropagation={() => dispatch('unstage', file.path)}
            >
              −
            </button>
          </div>
          <span class="stats">
            {#if file.additions > 0}
              <span class="add">+{file.additions}</span>
            {/if}
            {#if file.deletions > 0}
              <span class="del">−{file.deletions}</span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {/if}

  {#if unstagedFiles.length > 0}
    <div class="section">
      <div class="section-header">
        <span class="section-title">Changes</span>
        <span class="count">{unstagedFiles.length}</span>
      </div>
      {#each unstagedFiles as file}
        <div
          class="file-item"
          class:selected={selectedFile === file.path}
          role="button"
          tabindex="0"
          on:click={() => dispatch('select', file.path)}
          on:keydown={(e) => e.key === 'Enter' && dispatch('select', file.path)}
        >
          <span class="status" style="color: {getStatusColor(file.status)}">
            {getStatusIcon(file.status)}
          </span>
          <span class="path">
            <span class="dir">{getDirectory(file.path)}</span>
            <span class="name">{getFileName(file.path)}</span>
          </span>
          <div class="actions">
            <button 
              class="action-btn" 
              title="Stage"
              on:click|stopPropagation={() => dispatch('stage', file.path)}
            >
              +
            </button>
          </div>
          <span class="stats">
            {#if file.additions > 0}
              <span class="add">+{file.additions}</span>
            {/if}
            {#if file.deletions > 0}
              <span class="del">−{file.deletions}</span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {/if}

  {#if files.length === 0}
    <div class="empty">
      <p>No changes</p>
    </div>
  {/if}
</div>

<style>
  .file-tree {
    padding: 8px 0;
  }

  .section {
    margin-bottom: 16px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .count {
    background: var(--bg-tertiary);
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 11px;
  }

  .file-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 16px;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    font-size: 13px;
  }

  .file-item:hover {
    background: var(--bg-tertiary);
  }

  .file-item.selected {
    background: rgba(88, 166, 255, 0.15);
  }

  .status {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    width: 14px;
    text-align: center;
  }

  .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .dir {
    color: var(--text-muted);
  }

  .name {
    color: var(--text-primary);
  }

  .actions {
    opacity: 0;
  }

  .file-item:hover .actions {
    opacity: 1;
  }

  .action-btn {
    padding: 2px 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 14px;
    font-weight: bold;
    line-height: 1;
  }

  .action-btn:hover {
    background: var(--accent-blue);
    color: white;
    border-color: var(--accent-blue);
  }

  .stats {
    font-family: var(--font-mono);
    font-size: 11px;
    display: flex;
    gap: 4px;
  }

  .add {
    color: var(--accent-green);
  }

  .del {
    color: var(--accent-red);
  }

  .empty {
    padding: 32px 16px;
    text-align: center;
    color: var(--text-muted);
  }
</style>
