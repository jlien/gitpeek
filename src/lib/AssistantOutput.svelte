<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export interface AssistantRun {
    id: number;
    file: string;
    line: number;
    prompt: string;
    status: 'running' | 'success' | 'error';
    output: string;
  }

  export let runs: AssistantRun[] = [];

  const dispatch = createEventDispatcher();

  function fileName(path: string) {
    return path.split('/').pop() ?? path;
  }
</script>

<aside class="output-panel">
  <div class="panel-header">
    <span class="panel-title">Assistant Output</span>
    <button class="close-btn" on:click={() => dispatch('close')} title="Close">✕</button>
  </div>

  <div class="runs">
    {#each runs as run (run.id)}
      <div class="run" class:running={run.status === 'running'} class:error={run.status === 'error'}>
        <div class="run-meta">
          <span class="status-dot status-{run.status}"></span>
          <span class="run-location">{fileName(run.file)}:{run.line}</span>
          {#if run.status === 'running'}
            <span class="run-status">running…</span>
          {:else if run.status === 'error'}
            <span class="run-status error-text">failed</span>
          {/if}
        </div>
        <p class="run-prompt">{run.prompt}</p>
        {#if run.output}
          <pre class="run-output">{run.output}</pre>
        {/if}
      </div>
    {/each}
  </div>
</aside>

<style>
  .output-panel {
    width: 320px;
    min-width: 220px;
    border-left: 1px solid var(--border-color);
    background: var(--bg-secondary);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  .panel-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 13px;
    padding: 2px 4px;
    line-height: 1;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .runs {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }

  .run {
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .run-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-running {
    background: var(--accent-blue);
    animation: pulse 1s ease-in-out infinite;
  }

  .status-success {
    background: var(--accent-green);
  }

  .status-error {
    background: var(--accent-red);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .run-location {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }

  .run-status {
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }

  .error-text {
    color: var(--accent-red);
  }

  .run-prompt {
    margin: 0 0 6px 0;
    font-size: 12px;
    color: var(--text-primary);
    line-height: 1.4;
  }

  .run-output {
    margin: 0;
    padding: 8px;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 300px;
    overflow-y: auto;
    line-height: 1.5;
  }
</style>
