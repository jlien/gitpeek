<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import hljs from 'highlight.js/lib/core';
  import javascript from 'highlight.js/lib/languages/javascript';
  import typescript from 'highlight.js/lib/languages/typescript';
  import python from 'highlight.js/lib/languages/python';
  import ruby from 'highlight.js/lib/languages/ruby';
  import rust from 'highlight.js/lib/languages/rust';
  import xml from 'highlight.js/lib/languages/xml';
  import 'highlight.js/styles/github-dark.css';

  hljs.registerLanguage('xml', xml);
  hljs.registerLanguage('javascript', javascript);
  hljs.registerLanguage('typescript', typescript);
  hljs.registerLanguage('python', python);
  hljs.registerLanguage('ruby', ruby);
  hljs.registerLanguage('rust', rust);

  export let diff: string = '';
  export let viewMode: 'split' | 'unified' = 'split';
  export let pendingLines: Set<number> = new Set();
  export let filePath: string = '';

  const dispatch = createEventDispatcher();

  interface DiffLine {
    type: 'context' | 'add' | 'delete' | 'header' | 'hunk';
    content: string;
    oldLineNo?: number;
    newLineNo?: number;
    preHighlighted?: boolean; // content is already escaped/highlighted HTML
  }

  interface DiffHunk {
    header: string;
    lines: DiffLine[];
    newStart: number;
    newEnd: number;
    oldStart: number;
    oldEnd: number;
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
        currentHunk = { header: line, lines: [], newStart: newLine + 1, newEnd: newLine, oldStart: oldLine + 1, oldEnd: oldLine };
        hunks.push(currentHunk);
        currentHunk.lines.push({ type: 'hunk', content: line });
      } else if (currentHunk) {
        if (line.startsWith('+') && !line.startsWith('+++')) {
          newLine++;
          currentHunk.newEnd = newLine;
          currentHunk.lines.push({ type: 'add', content: line.slice(1), newLineNo: newLine });
        } else if (line.startsWith('-') && !line.startsWith('---')) {
          oldLine++;
          currentHunk.oldEnd = oldLine;
          currentHunk.lines.push({ type: 'delete', content: line.slice(1), oldLineNo: oldLine });
        } else if (line.startsWith(' ')) {
          oldLine++;
          newLine++;
          currentHunk.newEnd = newLine;
          currentHunk.oldEnd = oldLine;
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

  // ── Syntax highlighting ───────────────────────────────────────────────────

  function getLanguage(path: string): string | null {
    const ext = path.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'js': case 'jsx': case 'mjs': case 'cjs': return 'javascript';
      case 'ts': case 'tsx': case 'svelte': return 'typescript';
      case 'py': case 'pyw': return 'python';
      case 'rb': return 'ruby';
      case 'rs': return 'rust';
      default: return null;
    }
  }

  function escapeHtml(s: string): string {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // Split highlight.js HTML output by newline while keeping span tags balanced.
  function splitHighlightedHtml(html: string): string[] {
    const lines: string[] = [];
    let current = '';
    const openTags: string[] = [];
    const re = /<span[^>]*>|<\/span>|\n|[^<\n]+|<[^>]*>/g;
    let match: RegExpExecArray | null;

    while ((match = re.exec(html)) !== null) {
      const token = match[0];
      if (token === '\n') {
        lines.push(current + openTags.map(() => '</span>').join(''));
        current = [...openTags].join('');
      } else if (token.startsWith('<span')) {
        openTags.push(token);
        current += token;
      } else if (token === '</span>') {
        openTags.pop();
        current += token;
      } else {
        current += token;
      }
    }
    if (current || openTags.length) {
      lines.push(current + openTags.map(() => '</span>').join(''));
    }
    return lines;
  }

  // Returns a map of `old:<lineNo>` / `new:<lineNo>` → highlighted HTML per hunk.
  function computeHighlightMap(hunks: DiffHunk[], lang: string | null): Map<string, string> {
    if (!lang) return new Map();
    const map = new Map<string, string>();
    for (const hunk of hunks) {
      const codeLines: string[] = [];
      const keys: string[] = [];
      for (const line of hunk.lines) {
        if (line.type === 'context' || line.type === 'add' || line.type === 'delete') {
          codeLines.push(line.content);
          const key = line.type === 'delete'
            ? `old:${line.oldLineNo}`
            : `new:${line.newLineNo ?? line.oldLineNo}`;
          keys.push(key);
        }
      }
      if (codeLines.length === 0) continue;
      const highlighted = hljs.highlight(codeLines.join('\n'), { language: lang, ignoreIllegals: true }).value;
      const splitLines = splitHighlightedHtml(highlighted);
      for (let i = 0; i < keys.length; i++) {
        map.set(keys[i], splitLines[i] ?? escapeHtml(codeLines[i]));
      }
    }
    return map;
  }

  $: hunks = parseDiff(diff);
  $: highlightMap = computeHighlightMap(hunks, getLanguage(filePath));
  // Reset expansion state whenever the diff changes
  $: diff, expandedAbove = new Map<number, DiffLine[]>();

  // ── Hunk expansion ────────────────────────────────────────────────────────

  // Maps hunk index → extra context lines to show above that hunk
  let expandedAbove = new Map<number, DiffLine[]>();

  async function toggleHunkExpansion(hunkIndex: number) {
    if (expandedAbove.has(hunkIndex)) {
      expandedAbove = new Map(expandedAbove);
      expandedAbove.delete(hunkIndex);
      return;
    }

    const hunk = hunks[hunkIndex];
    const prev = hunkIndex > 0 ? hunks[hunkIndex - 1] : null;
    const gapStart = prev ? prev.newEnd + 1 : 1;
    const gapEnd = hunk.newStart - 1;
    if (gapEnd < gapStart) return; // no gap

    const fetchStart = Math.max(gapStart, gapEnd - 49); // at most 50 lines
    try {
      const raw = await invoke<string[]>('get_file_lines', {
        path: filePath,
        start: fetchStart,
        end: gapEnd,
      });
      const lang = getLanguage(filePath);
      let highlighted: string[] = raw.map(escapeHtml);
      if (lang) {
        const html = hljs.highlight(raw.join('\n'), { language: lang, ignoreIllegals: true }).value;
        highlighted = splitHighlightedHtml(html);
      }
      const extra: DiffLine[] = raw.map((content, i) => ({
        type: 'context' as const,
        content: highlighted[i] ?? escapeHtml(content),
        oldLineNo: fetchStart + i,
        newLineNo: fetchStart + i,
        preHighlighted: true,
      }));
      expandedAbove = new Map(expandedAbove).set(hunkIndex, extra);
    } catch {
      // file may not exist in workdir (e.g. commit/branch view) — silently ignore
    }
  }

  function lineHtml(line: DiffLine): string {
    if (line.preHighlighted) return line.content;
    if (line.type === 'header' || line.type === 'hunk') return escapeHtml(line.content);
    const key = line.type === 'delete'
      ? `old:${line.oldLineNo}`
      : `new:${line.newLineNo ?? line.oldLineNo}`;
    return highlightMap.get(key) ?? escapeHtml(line.content);
  }

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

  // JS-based hover tracking — CSS :hover inside a scroll container doesn't
  // reliably deactivate in WebKit/Tauri.
  let hoveredKey: number | null = null;

  // ── Split view resizing ───────────────────────────────────────────────────

  let splitRatio = 0.5;
  let splitViewEl: HTMLDivElement;
  let isDragging = false;

  // ── Split column scroll sync ──────────────────────────────────────────────

  let leftColEl: HTMLDivElement;
  let rightColEl: HTMLDivElement;
  let isSyncingScroll = false;

  function onColScroll(e: Event, source: 'left' | 'right') {
    if (isSyncingScroll) return;
    isSyncingScroll = true;
    const scrollTop = (e.target as HTMLElement).scrollTop;
    if (source === 'left') rightColEl.scrollTop = scrollTop;
    else leftColEl.scrollTop = scrollTop;
    isSyncingScroll = false;
  }

  // Use a Svelte action so native listeners are attached directly to the node.
  // Svelte still instruments the `splitRatio`/`isDragging` assignments here
  // because they reference component-scope `let` variables.
  function resizeHandle(node: HTMLButtonElement) {
    function onPointerDown(e: PointerEvent) {
      e.preventDefault();
      node.setPointerCapture(e.pointerId);
      isDragging = true;
    }
    function onPointerMove(e: PointerEvent) {
      if (!isDragging || !splitViewEl) return;
      const rect = splitViewEl.getBoundingClientRect();
      splitRatio = Math.min(0.85, Math.max(0.15, (e.clientX - rect.left) / rect.width));
    }
    function onPointerUp() {
      isDragging = false;
    }
    node.addEventListener('pointerdown', onPointerDown);
    node.addEventListener('pointermove', onPointerMove);
    node.addEventListener('pointerup', onPointerUp);
    node.addEventListener('pointercancel', onPointerUp);
    return {
      destroy() {
        node.removeEventListener('pointerdown', onPointerDown);
        node.removeEventListener('pointermove', onPointerMove);
        node.removeEventListener('pointerup', onPointerUp);
        node.removeEventListener('pointercancel', onPointerUp);
      }
    };
  }
</script>

<div class="diff-viewer" class:split={viewMode === 'split'}>
  {#if viewMode === 'unified'}
    <div class="unified-view">
      {#each hunks as hunk, hunkIndex}
        {#if expandedAbove.has(hunkIndex)}
          {#each expandedAbove.get(hunkIndex) ?? [] as xline}
            <div class="line context expanded-line">
              <span class="gutter"></span>
              <span class="line-no old">{xline.oldLineNo ?? ''}</span>
              <span class="line-no new">{xline.newLineNo ?? ''}</span>
              <span class="prefix"> </span>
              <span class="content">{@html lineHtml(xline)}</span>
            </div>
          {/each}
        {/if}
        {#each hunk.lines as line}
          {@const key = lineKey(line)}
          <div class="line {line.type}"
            class:is-pending={key !== undefined && pendingLines.has(key)}
            class:expandable={line.type === 'hunk'}
            on:mouseenter={() => { if (key !== undefined) hoveredKey = key; }}
            on:mouseleave={() => { if (key !== undefined && hoveredKey === key) hoveredKey = null; }}
            on:click={() => { if (line.type === 'hunk') toggleHunkExpansion(hunkIndex); }}
          >
            <span class="gutter">
              {#if isPromptable(line) && key !== undefined}
                {#if pendingLines.has(key)}
                  <span class="pending-dot" title="applying…"></span>
                {:else}
                  <button class="prompt-btn" class:visible={hoveredKey === key} title="Ask assistant" on:click|stopPropagation={() => openPrompt(key)}>
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
            <span class="content">{@html lineHtml(line)}</span>
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
    <div class="split-view" bind:this={splitViewEl} style="--split-left: {splitRatio * 100}%">
      <button
        class="split-divider"
        class:dragging={isDragging}
        use:resizeHandle
        aria-label="Resize panels"
      ></button>

      <!-- Left column -->
      <div class="split-col left" bind:this={leftColEl} on:scroll={(e) => onColScroll(e, 'left')}>
        {#each hunks as hunk, hunkIndex}
          {#if expandedAbove.has(hunkIndex)}
            {#each expandedAbove.get(hunkIndex) ?? [] as xline}
              <div class="split-line context expanded-line">
                <span class="line-no">{xline.oldLineNo ?? ''}</span>
                <span class="content">{@html lineHtml(xline)}</span>
              </div>
            {/each}
          {/if}
          {@const splitLines = getSplitLines(hunk.lines)}
          {#each splitLines as pair}
            <div class="split-line {pair.left?.type ?? 'empty'}"
              class:expandable={pair.left?.type === 'hunk'}
              on:click={() => { if (pair.left?.type === 'hunk') toggleHunkExpansion(hunkIndex); }}
            >
              {#if pair.left}
                {#if pair.left.type !== 'hunk' && pair.left.type !== 'header'}
                  <span class="line-no">{pair.left.oldLineNo ?? ''}</span>
                {/if}
                <span class="content">{@html lineHtml(pair.left)}</span>
              {:else}
                <span class="line-no"></span>
                <span class="content"></span>
              {/if}
            </div>
          {/each}
        {/each}
      </div>

      <!-- Right column -->
      <div class="split-col right" bind:this={rightColEl} on:scroll={(e) => onColScroll(e, 'right')}>
        {#each hunks as hunk, hunkIndex}
          {#if expandedAbove.has(hunkIndex)}
            {#each expandedAbove.get(hunkIndex) ?? [] as xline}
              <div class="split-line context expanded-line">
                <span class="gutter"></span>
                <span class="line-no">{xline.newLineNo ?? ''}</span>
                <span class="content">{@html lineHtml(xline)}</span>
              </div>
            {/each}
          {/if}
          {@const splitLines = getSplitLines(hunk.lines)}
          {#each splitLines as pair}
            {@const rightKey = pair.right ? lineKey(pair.right) : undefined}
            <div
              class="split-line {pair.right?.type ?? 'empty'}"
              class:is-pending={rightKey !== undefined && pendingLines.has(rightKey)}
              class:expandable={pair.right?.type === 'hunk'}
              on:mouseenter={() => { if (rightKey !== undefined) hoveredKey = rightKey; }}
              on:mouseleave={() => { if (rightKey !== undefined && hoveredKey === rightKey) hoveredKey = null; }}
              on:click={() => { if (pair.right?.type === 'hunk') toggleHunkExpansion(hunkIndex); }}
            >
              {#if pair.right}
                <span class="gutter">
                  {#if isPromptable(pair.right) && rightKey !== undefined}
                    {#if pendingLines.has(rightKey)}
                      <span class="pending-dot" title="applying…"></span>
                    {:else}
                      <button class="prompt-btn" class:visible={hoveredKey === rightKey} title="Ask assistant" on:click={() => openPrompt(rightKey)}>
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
                <span class="content">{@html lineHtml(pair.right)}</span>
              {:else}
                <span class="gutter"></span>
                <span class="line-no"></span>
                <span class="content"></span>
              {/if}
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
    min-width: max-content;
  }

  .line {
    display: flex;
    min-height: 20px;
    min-width: 100%;
    position: relative;
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

  .line.expandable {
    cursor: pointer;
  }

  .line.expandable:hover {
    background: rgba(88, 166, 255, 0.2);
  }

  .expanded-line {
    border-left: 2px solid var(--border-color);
    opacity: 0.75;
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
    pointer-events: none;
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

  .prompt-btn.visible {
    opacity: 1;
    pointer-events: auto;
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
    position: relative;
    overflow: hidden;
  }

  .split-divider {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(var(--split-left, 50%) - 4px);
    width: 8px;
    cursor: col-resize;
    z-index: 10;
    background: transparent;
    border: none;
    border-left: 2px solid transparent;
    padding: 0;
    transition: border-color 0.15s, background 0.15s;
  }

  .split-divider:hover,
  .split-divider.dragging {
    background: rgba(88, 166, 255, 0.15);
    border-left-color: var(--accent-blue);
  }

  /* Two-column layout: each column owns its own scroll */
  .split-col {
    display: flex;
    flex-direction: column;
    overflow: auto;
  }

  .split-col.left {
    flex-basis: var(--split-left, 50%);
    flex-shrink: 0;
    flex-grow: 0;
    border-right: 1px solid var(--border-color);
  }

  .split-col.right {
    flex: 1;
    min-width: 0;
  }

  .split-line {
    display: flex;
    min-height: 20px;
    min-width: max(100%, max-content);
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

  .split-line.expandable {
    cursor: pointer;
  }

  .split-line.expandable:hover {
    background: rgba(88, 166, 255, 0.2);
  }

  .split-line.expanded-line {
    border-left: 2px solid var(--border-color);
    opacity: 0.75;
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
  }

  .split-prompt-row {
    padding-left: 12px;
  }
</style>
