<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let diff: string = '';
  export let viewMode: 'split' | 'unified' = 'split';
  export let pendingLines: Set<number> = new Set();

  const dispatch = createEventDispatcher();

  interface DiffLine {
    type: 'context' | 'add' | 'delete' | 'header' | 'hunk';
    content: string;
    oldLineNo?: number;
    newLineNo?: number;
  }

  interface DiffHunk {
    header: string;
    lines: DiffLine[];
  }

  function parseDiff(raw: string): DiffHunk[] {
    const hunks: DiffHunk[] = [];
    const lines = raw.split('\n');
    let currentHunk: DiffHunk | null = null;
    let oldLine = 0;
    let newLine = 0;

    for (const line of lines) {
      if (line.startsWith('@@')) {
        const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
        if (match) {
          oldLine = parseInt(match[1], 10) - 1;
          newLine = parseInt(match[2], 10) - 1;
        }
        currentHunk = { header: line, lines: [] };
        hunks.push(currentHunk);
        currentHunk.lines.push({ type: 'hunk', content: line });
      } else if (currentHunk) {
        if (line.startsWith('+') && !line.startsWith('+++')) {
          newLine++;
          currentHunk.lines.push({ type: 'add', content: line.slice(1), newLineNo: newLine });
        } else if (line.startsWith('-') && !line.startsWith('---')) {
          oldLine++;
          currentHunk.lines.push({ type: 'delete', content: line.slice(1), oldLineNo: oldLine });
        } else if (line.startsWith(' ')) {
          oldLine++;
          newLine++;
          currentHunk.lines.push({ type: 'context', content: line.slice(1), oldLineNo: oldLine, newLineNo: newLine });
        } else if (
          line.startsWith('diff ') || line.startsWith('index ') ||
          line.startsWith('---') || line.startsWith('+++')
        ) {
          currentHunk.lines.push({ type: 'header', content: line });
        }
      }
    }

    return hunks;
  }

  $: hunks = parseDiff(diff);

  function getSplitLines(lines: DiffLine[]): Array<{ left: DiffLine | null; right: DiffLine | null }> {
    const result: Array<{ left: DiffLine | null; right: DiffLine | null }> = [];
    let i = 0;

    while (i < lines.length) {
      const line = lines[i];

      if (line.type === 'hunk' || line.type === 'header') {
        result.push({ left: line, right: line });
        i++;
      } else if (line.type === 'context') {
        result.push({ left: line, right: line });
        i++;
      } else if (line.type === 'delete') {
        const deletes: DiffLine[] = [];
        while (i < lines.length && lines[i].type === 'delete') {
          deletes.push(lines[i]);
          i++;
        }
        const adds: DiffLine[] = [];
        while (i < lines.length && lines[i].type === 'add') {
          adds.push(lines[i]);
          i++;
        }
        const maxLen = Math.max(deletes.length, adds.length);
        for (let j = 0; j < maxLen; j++) {
          result.push({ left: deletes[j] || null, right: adds[j] || null });
        }
      } else if (line.type === 'add') {
        result.push({ left: null, right: line });
        i++;
      } else {
        i++;
      }
    }

    return result;
  }

  // ── Inline prompt state ───────────────────────────────────────────────────

  let activePromptLine: number | null = null;
  let promptText = '';

  function lineKey(line: DiffLine): number | undefined {
    return line.newLineNo ?? line.oldLineNo;
  }

  function isPromptable(line: DiffLine): boolean {
    return line.type === 'add' || line.type === 'delete' || line.type === 'context';
  }

  function openPrompt(key: number) {
    activePromptLine = key;
    promptText = '';
  }

  function cancelPrompt() {
    activePromptLine = null;
    promptText = '';
  }

  function hunkContext(hunk: DiffHunk): string {
    return hunk.lines.map(l => {
      if (l.type === 'hunk') return l.content;
      const prefix = l.type === 'add' ? '+' : l.type === 'delete' ? '-' : ' ';
      return prefix + l.content;
    }).join('\n');
  }

  function submitPrompt(key: number, hunk: DiffHunk) {
    const text = promptText.trim();
    if (!text) return;
    dispatch('promptSubmit', { line: key, prompt: text, context: hunkContext(hunk) });
    promptText = '';
    activePromptLine = null;
  }

  function onPromptKeydown(e: KeyboardEvent, key: number, hunk: DiffHunk) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) submitPrompt(key, hunk);
    if (e.key === 'Escape') cancelPrompt();
  }
</script>

<div class="diff-viewer" class:split={viewMode === 'split'}>
  {#if viewMode === 'unified'}
    <div class="unified-view">
      {#each hunks as hunk}
        {#each hunk.lines as line}
          {@const key = lineKey(line)}
          <div class="line {line.type}" class:is-pending={key !== undefined && pendingLines.has(key)}>
            <span class="gutter">
              {#if isPromptable(line) && key !== undefined}
                {#if pendingLines.has(key)}
                  <span class="pending-dot" title="applying…"></span>
                {:else}
                  <button class="prompt-btn" title="Ask assistant" on:click={() => openPrompt(key)}>
                    <svg viewBox="0 0 16 16" width="11" height="11" fill="currentColor">
                      <path d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0 1 13.25 12H9.06l-2.573 2.573A1.457 1.457 0 0 1 4 13.543V12H2.75A1.75 1.75 0 0 1 1 10.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h4.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>
                    </svg>
                  </button>
                {/if}
              {/if}
            </span>
            <span class="line-no old">{line.oldLineNo ?? ''}</span>
            <span class="line-no new">{line.newLineNo ?? ''}</span>
            <span class="prefix">
              {#if line.type === 'add'}+{:else if line.type === 'delete'}-{:else if line.type !== 'hunk' && line.type !== 'header'} {/if}
            </span>
            <span class="content">{line.content}</span>
          </div>
          {#if activePromptLine === key && key !== undefined}
            <div class="prompt-row">
              <textarea
                bind:value={promptText}
                placeholder="Describe the change… (⌘↵ to apply)"
                rows="2"
                on:keydown={(e) => onPromptKeydown(e, key, hunk)}
                autofocus
              ></textarea>
              <div class="prompt-actions">
                <button on:click={cancelPrompt}>Cancel</button>
                <button class="apply" on:click={() => submitPrompt(key, hunk)} disabled={!promptText.trim()}>
                  Apply
                </button>
              </div>
            </div>
          {/if}
        {/each}
      {/each}
    </div>
  {:else}
    <div class="split-view">
      <div class="split-container">
        {#each hunks as hunk}
          {@const splitLines = getSplitLines(hunk.lines)}
          {#each splitLines as pair}
            {@const rightKey = pair.right ? lineKey(pair.right) : undefined}
            <div class="split-row">
              <div class="split-line left {pair.left?.type ?? 'empty'}">
                {#if pair.left}
                  {#if pair.left.type !== 'hunk' && pair.left.type !== 'header'}
                    <span class="line-no">{pair.left.oldLineNo ?? ''}</span>
                  {/if}
                  <span class="content">{pair.left.content}</span>
                {:else}
                  <span class="line-no"></span>
                  <span class="content"></span>
                {/if}
              </div>
              <div
                class="split-line right {pair.right?.type ?? 'empty'}"
                class:is-pending={rightKey !== undefined && pendingLines.has(rightKey)}
              >
                {#if pair.right}
                  <span class="gutter">
                    {#if pair.right && isPromptable(pair.right) && rightKey !== undefined}
                      {#if pendingLines.has(rightKey)}
                        <span class="pending-dot" title="applying…"></span>
                      {:else}
                        <button class="prompt-btn" title="Ask assistant" on:click={() => openPrompt(rightKey)}>
                          <svg viewBox="0 0 16 16" width="11" height="11" fill="currentColor">
                            <path d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0 1 13.25 12H9.06l-2.573 2.573A1.457 1.457 0 0 1 4 13.543V12H2.75A1.75 1.75 0 0 1 1 10.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h4.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>
                          </svg>
                        </button>
                      {/if}
                    {/if}
                  </span>
                  {#if pair.right.type !== 'hunk' && pair.right.type !== 'header'}
                    <span class="line-no">{pair.right.newLineNo ?? ''}</span>
                  {/if}
                  <span class="content">{pair.right.content}</span>
                {:else}
                  <span class="gutter"></span>
                  <span class="line-no"></span>
                  <span class="content"></span>
                {/if}
              </div>
            </div>
            {#if activePromptLine === rightKey && rightKey !== undefined}
              <div class="prompt-row split-prompt-row">
                <textarea
                  bind:value={promptText}
                  placeholder="Describe the change… (⌘↵ to apply)"
                  rows="2"
                  on:keydown={(e) => onPromptKeydown(e, rightKey, hunk)}
                  autofocus
                ></textarea>
                <div class="prompt-actions">
                  <button on:click={cancelPrompt}>Cancel</button>
                  <button class="apply" on:click={() => submitPrompt(rightKey, hunk)} disabled={!promptText.trim()}>
                    Apply
                  </button>
                </div>
              </div>
            {/if}
          {/each}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .diff-viewer {
    flex: 1;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 20px;
  }

  /* Unified View */
  .unified-view {
    padding: 0;
  }

  .line {
    display: flex;
    min-height: 20px;
    position: relative;
  }

  .line:hover .prompt-btn,
  .split-line.right:hover .prompt-btn {
    opacity: 1;
  }

  .line.add {
    background: var(--diff-add-bg);
  }

  .line.delete {
    background: var(--diff-del-bg);
  }

  .line.hunk {
    background: rgba(88, 166, 255, 0.1);
    color: var(--accent-blue);
    padding: 8px 0;
    margin: 8px 0;
  }

  .line.header {
    color: var(--text-muted);
    padding: 4px 0;
  }

  .line-no {
    width: 50px;
    padding: 0 8px;
    text-align: right;
    color: var(--text-muted);
    user-select: none;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-color);
  }

  .line-no.old {
    border-right: none;
  }

  .prefix {
    width: 20px;
    text-align: center;
    color: var(--text-muted);
    user-select: none;
  }

  .content {
    flex: 1;
    padding: 0 8px;
    white-space: pre;
  }

  /* Gutter column (holds prompt button or pending dot) */
  .gutter {
    width: 24px;
    min-width: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  .prompt-btn {
    opacity: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    transition: opacity 0.1s, color 0.1s;
  }

  .prompt-btn:hover {
    color: var(--accent-blue);
  }

  .pending-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent-blue);
    animation: pulse 1s ease-in-out infinite;
    flex-shrink: 0;
  }

  /* Inline prompt row */
  .prompt-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 12px 8px 120px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border-color);
    border-bottom: 1px solid var(--border-color);
  }

  .prompt-row textarea {
    flex: 1;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 6px 8px;
    resize: none;
    line-height: 1.5;
  }

  .prompt-row textarea:focus {
    outline: none;
    border-color: var(--accent-blue);
  }

  .prompt-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .prompt-actions button {
    padding: 4px 12px;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    border: 1px solid var(--border-color);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .prompt-actions button.apply {
    background: var(--accent-blue);
    border-color: var(--accent-blue);
    color: white;
  }

  .prompt-actions button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* Split View */
  .split-view {
    display: flex;
    height: 100%;
  }

  .split-container {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .split-row {
    display: flex;
    min-height: 20px;
  }

  .split-line {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .split-line.left {
    border-right: 1px solid var(--border-color);
  }

  .split-line.add {
    background: var(--diff-add-bg);
  }

  .split-line.delete {
    background: var(--diff-del-bg);
  }

  .split-line.empty {
    background: var(--bg-tertiary);
  }

  .split-line.hunk,
  .split-line.header {
    background: rgba(88, 166, 255, 0.1);
    color: var(--accent-blue);
  }

  .split-line .line-no {
    width: 40px;
    min-width: 40px;
    padding: 0 8px;
    text-align: right;
    color: var(--text-muted);
    user-select: none;
    background: inherit;
    border-right: 1px solid var(--border-color);
  }

  .split-line .content {
    flex: 1;
    padding: 0 8px;
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .split-prompt-row {
    padding-left: 12px;
  }
</style>
