<script lang="ts">
  import './app.css';
  import FileTree from './lib/FileTree.svelte';
  import DiffViewer from './lib/DiffViewer.svelte';
  import Header from './lib/Header.svelte';
  import AssistantConfig from './lib/AssistantConfig.svelte';
  import AssistantOutput from './lib/AssistantOutput.svelte';
  import type { AssistantRun } from './lib/AssistantOutput.svelte';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import CommitLog from './lib/CommitLog.svelte';
  import type { CommitInfo } from './lib/CommitLog.svelte';
  import BranchDiff from './lib/BranchDiff.svelte';

  interface FileChange {
    path: string;
    status: 'added' | 'modified' | 'deleted' | 'renamed';
    staged: boolean;
    additions: number;
    deletions: number;
  }

  interface RepoInfo {
    path: string;
    branch: string;
    remote: string | null;
  }

  let repoInfo: RepoInfo | null = null;
  let files: FileChange[] = [];
  let selectedFile: string | null = null;
  let diff: string = '';
  let loading = true;
  let error: string | null = null;
  let viewMode: 'split' | 'unified' = 'split';
  let showConfig = false;
  let showOutput = false;
  let runs: AssistantRun[] = [];
  let nextRunId = 0;

  // Sidebar mode
  type SidebarMode = 'changes' | 'commits' | 'branch';
  let sidebarMode: SidebarMode = 'changes';
  let commits: CommitInfo[] = [];
  let selectedCommitHash: string | null = null;
  let commitsLoaded = false;
  let branchRefreshTick = 0;
  let selectedBranchBase: string | null = null;

  // pending lines per file: Map<filePath, Set<lineNo>>
  let pendingByFile: Map<string, Set<number>> = new Map();
  $: pendingLines = pendingByFile.get(selectedFile ?? '') ?? new Set<number>();

  async function loadRepo(path?: string) {
    loading = true;
    error = null;
    commitsLoaded = false;
    commits = [];
    try {
      repoInfo = await invoke('get_repo_info', { path });
      await refreshFiles();
    } catch (e) {
      error = String(e);
    }
    loading = false;
  }

  async function loadCommits() {
    if (commitsLoaded) return;
    try {
      commits = await invoke('get_commits', { limit: 100 });
      commitsLoaded = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function switchSidebar(mode: SidebarMode) {
    sidebarMode = mode;
    if (mode === 'commits') await loadCommits();
  }

  async function handleCommitFileSelect(e: CustomEvent<{ hash: string; path: string }>) {
    const { hash, path } = e.detail;
    selectedCommitHash = hash;
    selectedFile = path;
    try {
      diff = await invoke('get_commit_file_diff', { hash, path });
    } catch (err) {
      diff = `Error loading diff: ${err}`;
    }
  }

  async function handleBranchFileSelect(e: CustomEvent<{ base: string; head: string; path: string }>) {
    const { base, head, path } = e.detail;
    selectedBranchBase = base;
    selectedCommitHash = null;
    selectedFile = path;
    try {
      diff = await invoke('get_branch_file_diff', { base, head, path });
    } catch (err) {
      diff = `Error loading diff: ${err}`;
    }
  }

  async function refreshFiles() {
    try {
      files = await invoke('get_changed_files');
      if (files.length > 0 && !selectedFile) {
        await selectFile(files[0].path);
      }
    } catch (e) {
      error = String(e);
    }
    if (commitsLoaded) {
      commitsLoaded = false;
      await loadCommits();
    }
    if (sidebarMode === 'branch') {
      branchRefreshTick += 1;
    }
  }

  async function selectFile(path: string) {
    selectedFile = path;
    try {
      diff = await invoke('get_file_diff', { path });
    } catch (e) {
      diff = `Error loading diff: ${e}`;
    }
  }

  async function stageFile(path: string) {
    await invoke('stage_file', { path });
    await refreshFiles();
  }

  async function unstageFile(path: string) {
    await invoke('unstage_file', { path });
    await refreshFiles();
  }

  async function commitStaged(message: string) {
    try {
      await invoke('commit_staged', { message });
      await refreshFiles();
    } catch (e) {
      error = String(e);
    }
  }

  async function openFolder() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({ directory: true });
    if (selected) {
      await loadRepo(selected as string);
    }
  }

  async function handlePromptSubmit(
    e: CustomEvent<{ line: number; prompt: string; context: string }>
  ) {
    if (!selectedFile) return;
    const { line, prompt, context } = e.detail;
    const file = selectedFile;

    // Add a running entry to the output panel and show it
    const runId = nextRunId++;
    runs = [{ id: runId, file, line, prompt, status: 'running', output: '' }, ...runs];
    showOutput = true;

    // Mark line as pending
    const existing = new Set(pendingByFile.get(file) ?? []);
    existing.add(line);
    pendingByFile = new Map(pendingByFile).set(file, existing);

    try {
      const output = await invoke<string>('run_assistant', {
        prompt,
        filePath: file,
        line,
        diffContext: context,
      });
      runs = runs.map(r => r.id === runId ? { ...r, status: 'success', output } : r);
      await refreshFiles();
      if (files.some(f => f.path === file)) {
        await selectFile(file);
      }
    } catch (err) {
      runs = runs.map(r => r.id === runId ? { ...r, status: 'error', output: String(err) } : r);
    } finally {
      const lines = new Set(pendingByFile.get(file) ?? []);
      lines.delete(line);
      pendingByFile = new Map(pendingByFile).set(file, lines);
    }
  }

  // ── Diff stats ────────────────────────────────────────────────────────────
  $: diffStats = (() => {
    let additions = 0, deletions = 0;
    for (const line of diff.split('\n')) {
      if (line.startsWith('+') && !line.startsWith('+++')) additions++;
      else if (line.startsWith('-') && !line.startsWith('---')) deletions++;
    }
    return { additions, deletions };
  })();

  // ── Sidebar resize ────────────────────────────────────────────────────────
  let sidebarWidth = 300;
  let isResizing = false;

  function onResizePointerDown(e: PointerEvent) {
    e.preventDefault();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    isResizing = true;
  }

  function onResizePointerMove(e: PointerEvent) {
    if (!isResizing) return;
    sidebarWidth = Math.min(600, Math.max(160, e.clientX));
  }

  function onResizePointerUp() {
    isResizing = false;
  }

  onMount(async () => {
    const lastRepo = await invoke<string | null>('get_last_repo').catch(() => null);
    loadRepo(lastRepo ?? undefined);

    const unOpen   = await listen('menu-open', () => openFolder());
    const unRecent = await listen<string>('menu-open-recent', (e) => loadRepo(e.payload));
    return () => { unOpen(); unRecent(); };
  });
</script>

<main>
  <Header
    {repoInfo}
    on:refresh={refreshFiles}
    on:openFolder={openFolder}
    on:configure={() => showConfig = true}
  />

  {#if showConfig}
    <AssistantConfig on:close={() => showConfig = false} />
  {/if}

  <div class="container">
    <aside class="sidebar" style="width: {sidebarWidth}px">
      <div class="sidebar-tabs">
        <button
          class="sidebar-tab"
          class:active={sidebarMode === 'changes'}
          on:click={() => switchSidebar('changes')}
        >Changes</button>
        <button
          class="sidebar-tab"
          class:active={sidebarMode === 'commits'}
          on:click={() => switchSidebar('commits')}
        >Log</button>
        <button
          class="sidebar-tab"
          class:active={sidebarMode === 'branch'}
          on:click={() => switchSidebar('branch')}
        >Branch</button>
      </div>

      {#if sidebarMode === 'changes'}
        <FileTree
          {files}
          {selectedFile}
          on:select={(e) => selectFile(e.detail)}
          on:stage={(e) => stageFile(e.detail)}
          on:unstage={(e) => unstageFile(e.detail)}
          on:commit={(e) => commitStaged(e.detail)}
        />
      {:else if sidebarMode === 'commits'}
        <CommitLog
          {commits}
          selectedHash={selectedCommitHash}
          {selectedFile}
          on:selectFile={handleCommitFileSelect}
        />
      {:else}
        <BranchDiff
          currentBranch={repoInfo?.branch ?? ''}
          {selectedFile}
          selectedBase={selectedBranchBase}
          refreshTick={branchRefreshTick}
          on:selectFile={handleBranchFileSelect}
        />
      {/if}

      <div
        class="sidebar-resize-handle"
        class:resizing={isResizing}
        on:pointerdown={onResizePointerDown}
        on:pointermove={onResizePointerMove}
        on:pointerup={onResizePointerUp}
        on:pointercancel={onResizePointerUp}
      ></div>
    </aside>

    <section class="main-content">
      {#if loading}
        <div class="loading">Loading repository...</div>
      {:else if error}
        <div class="error">
          <h3>Error</h3>
          <p>{error}</p>
          <button on:click={openFolder}>Open Repository</button>
        </div>
      {:else if selectedFile}
        <div class="diff-header">
          <span class="filename">
            {#if sidebarMode === 'commits' && selectedCommitHash}
              <span class="commit-ref">{selectedCommitHash.slice(0, 7)}</span>
            {:else if sidebarMode === 'branch' && selectedBranchBase}
              <span class="commit-ref">{selectedBranchBase}…{repoInfo?.branch}</span>
            {/if}
            {selectedFile}
          </span>
          <div class="diff-header-right">
            {#if diffStats.additions > 0 || diffStats.deletions > 0}
              <span class="diff-stats">
                {#if diffStats.additions > 0}<span class="stat-add">+{diffStats.additions}</span>{/if}
                {#if diffStats.deletions > 0}<span class="stat-del">−{diffStats.deletions}</span>{/if}
              </span>
            {/if}
            <div class="view-toggle">
              <button
                class:active={viewMode === 'split'}
                on:click={() => viewMode = 'split'}
              >
                Split
              </button>
              <button
                class:active={viewMode === 'unified'}
                on:click={() => viewMode = 'unified'}
              >
                Unified
              </button>
            </div>
          </div>
        </div>
        <div class="diff-and-output">
          <DiffViewer
            {diff}
            {viewMode}
            {pendingLines}
            filePath={selectedFile ?? ''}
            on:promptSubmit={handlePromptSubmit}
          />
          {#if showOutput}
            <AssistantOutput {runs} on:close={() => showOutput = false} />
          {/if}
        </div>
      {:else}
        <div class="empty">
          <p>No changes detected</p>
          <button on:click={openFolder}>Open Repository</button>
        </div>
      {/if}
    </section>
  </div>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .container {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .sidebar {
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-color);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    position: relative;
    flex-shrink: 0;
  }

  .sidebar-resize-handle {
    position: absolute;
    top: 0;
    right: -3px;
    width: 6px;
    height: 100%;
    cursor: col-resize;
    z-index: 10;
  }

  .sidebar-resize-handle:hover,
  .sidebar-resize-handle.resizing {
    background: var(--accent-blue);
    opacity: 0.5;
  }

  .sidebar-tabs {
    display: flex;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  .sidebar-tab {
    flex: 1;
    padding: 8px 0;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    margin-bottom: -1px;
  }

  .sidebar-tab:hover {
    color: var(--text-primary);
  }

  .sidebar-tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--accent-blue);
  }

  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .diff-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
  }

  .filename {
    font-family: var(--font-mono);
    font-size: 14px;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .commit-ref {
    font-size: 11px;
    color: var(--accent-blue);
    background: rgba(88, 166, 255, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .diff-header-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .diff-stats {
    display: flex;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
  }

  .stat-add {
    color: var(--accent-green);
  }

  .stat-del {
    color: var(--accent-red);
  }

  .view-toggle {
    display: flex;
    gap: 4px;
  }

  .view-toggle button {
    padding: 4px 12px;
    border: 1px solid var(--border-color);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
  }

  .view-toggle button:first-child {
    border-radius: 6px 0 0 6px;
  }

  .view-toggle button:last-child {
    border-radius: 0 6px 6px 0;
  }

  .view-toggle button.active {
    background: var(--accent-blue);
    color: white;
    border-color: var(--accent-blue);
  }

  .diff-and-output {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .loading, .error, .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: var(--text-secondary);
  }

  .error h3 {
    color: var(--accent-red);
  }

  button {
    padding: 8px 16px;
    background: var(--accent-blue);
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-size: 14px;
  }

  button:hover {
    opacity: 0.9;
  }
</style>
