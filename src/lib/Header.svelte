<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let repoInfo: { path: string; branch: string; remote: string | null } | null = null;
  export let branches: { name: string; remote: string | null }[] = [];

  const dispatch = createEventDispatcher();

  let showBranchMenu = false;
  let fetching = false;

  $: localBranches = branches.filter(b => !b.remote);
  $: remoteBranches = branches.filter(b => !!b.remote);

  function onBranchSelect(b: { name: string; remote: string | null }) {
    showBranchMenu = false;
    if (b.remote || b.name !== repoInfo?.branch) {
      dispatch('checkout', { branch: b.name, remote: b.remote ?? undefined });
    }
  }

  function onBranchKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') showBranchMenu = false;
  }

  async function onFetch() {
    fetching = true;
    dispatch('fetch');
    // parent will reload branches; just show spinner briefly
    await new Promise(r => setTimeout(r, 300));
    fetching = false;
  }
</script>

<header>
  <div class="left">
    <span class="logo">🔍 GitPeek</span>
    {#if repoInfo}
      <span class="divider">/</span>
      <span class="repo-path" title={repoInfo.path}>
        {repoInfo.path.split('/').slice(-2).join('/')}
      </span>
      <div class="branch-wrap" on:keydown={onBranchKeydown}>
        <button
          class="branch"
          class:open={showBranchMenu}
          on:click={() => showBranchMenu = !showBranchMenu}
          title="Switch branch"
        >
          <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor">
            <path d="M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.5 2.5 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Z"/>
          </svg>
          {repoInfo.branch}
          <svg class="caret" viewBox="0 0 16 16" width="10" height="10" fill="currentColor">
            <path d="m4.427 7.427 3.396 3.396a.25.25 0 0 0 .354 0l3.396-3.396A.25.25 0 0 0 11.396 7H4.604a.25.25 0 0 0-.177.427Z"/>
          </svg>
        </button>
        {#if showBranchMenu && branches.length > 0}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <div class="branch-backdrop" on:click={() => showBranchMenu = false} role="none"></div>
          <div class="branch-menu" role="listbox">
            <button class="fetch-btn" on:click={onFetch} disabled={fetching}>
              {#if fetching}
                Fetching…
              {:else}
                <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor"><path d="M8 2.5a5.487 5.487 0 0 0-4.131 1.869l1.204 1.204A.25.25 0 0 1 4.896 6H1.25A.25.25 0 0 1 1 5.75V2.104a.25.25 0 0 1 .427-.177l1.38 1.38A7.002 7.002 0 0 1 14.95 7.16a.75.75 0 0 1-1.49.178A5.5 5.5 0 0 0 8 2.5ZM1.705 8.005a.75.75 0 0 1 .834.656 5.5 5.5 0 0 0 9.592 2.97l-1.204-1.204a.25.25 0 0 1 .177-.427h3.646a.25.25 0 0 1 .25.25v3.646a.25.25 0 0 1-.427.177l-1.38-1.38A7.002 7.002 0 0 1 1.05 8.84a.75.75 0 0 1 .656-.834Z"/></svg>
                Fetch
              {/if}
            </button>
            {#if localBranches.length > 0}
              <div class="branch-group-label">Local</div>
              {#each localBranches as b}
                <button
                  class="branch-option"
                  class:current={b.name === repoInfo?.branch}
                  role="option"
                  aria-selected={b.name === repoInfo?.branch}
                  on:click={() => onBranchSelect(b)}
                >{b.name}</button>
              {/each}
            {/if}
            {#if remoteBranches.length > 0}
              <div class="branch-group-label">Remote</div>
              {#each remoteBranches as b}
                <button
                  class="branch-option remote"
                  role="option"
                  aria-selected={false}
                  on:click={() => onBranchSelect(b)}
                >{b.name}</button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
  
  <div class="right">
    <button class="icon-btn" on:click={() => dispatch('ask')} title="Ask assistant">
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor">
        <path d="M0 2.75C0 1.784.784 1 1.75 1h12.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0 1 14.25 12H8.061l-2.574 2.573A1.458 1.458 0 0 1 3 13.543V12H1.75A1.75 1.75 0 0 1 0 10.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"/>
      </svg>
    </button>
    <button class="icon-btn" on:click={() => dispatch('refresh')} title="Refresh">
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor">
        <path d="M1.705 8.005a.75.75 0 0 1 .834.656 5.5 5.5 0 0 0 9.592 2.97l-1.204-1.204a.25.25 0 0 1 .177-.427h3.646a.25.25 0 0 1 .25.25v3.646a.25.25 0 0 1-.427.177l-1.38-1.38A7.002 7.002 0 0 1 1.05 8.84a.75.75 0 0 1 .656-.834ZM8 2.5a5.487 5.487 0 0 0-4.131 1.869l1.204 1.204A.25.25 0 0 1 4.896 6H1.25A.25.25 0 0 1 1 5.75V2.104a.25.25 0 0 1 .427-.177l1.38 1.38A7.002 7.002 0 0 1 14.95 7.16a.75.75 0 0 1-1.49.178A5.5 5.5 0 0 0 8 2.5Z"/>
      </svg>
    </button>
    <button class="icon-btn" on:click={() => dispatch('openFolder')} title="Open Folder">
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor">
        <path d="M1.75 1A1.75 1.75 0 0 0 0 2.75v10.5C0 14.216.784 15 1.75 15h12.5A1.75 1.75 0 0 0 16 13.25v-8.5A1.75 1.75 0 0 0 14.25 3H7.5a.25.25 0 0 1-.2-.1l-.9-1.2C6.07 1.26 5.55 1 5 1H1.75Z"/>
      </svg>
    </button>
    <button class="icon-btn" on:click={() => dispatch('configure')} title="Assistant Settings">
      <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor">
        <path d="M8 0a8.2 8.2 0 0 1 .701.031C9.444.095 9.99.645 10.16 1.29l.288 1.107c.018.066.079.158.212.224.231.114.454.243.668.386.123.082.233.09.299.071l1.103-.303c.644-.176 1.392.021 1.82.63.27.385.506.792.704 1.218.315.675.111 1.422-.364 1.891l-.814.806c-.049.048-.098.147-.088.294.016.257.016.515 0 .772-.01.147.038.246.088.294l.814.806c.475.469.679 1.216.364 1.891a7.977 7.977 0 0 1-.704 1.217c-.428.61-1.176.807-1.82.63l-1.102-.302c-.067-.019-.177-.011-.3.071a5.909 5.909 0 0 1-.668.386c-.133.066-.194.158-.211.224l-.29 1.106c-.168.646-.715 1.196-1.458 1.26a8.006 8.006 0 0 1-1.402 0c-.743-.064-1.289-.614-1.458-1.26l-.289-1.106c-.018-.066-.079-.158-.212-.224a5.738 5.738 0 0 1-.668-.386c-.123-.082-.233-.09-.299-.071l-1.103.303c-.644.176-1.392-.021-1.82-.63a8.12 8.12 0 0 1-.704-1.218c-.315-.675-.111-1.422.363-1.891l.815-.806c.05-.048.098-.147.088-.294a6.214 6.214 0 0 1 0-.772c.01-.147-.038-.246-.088-.294l-.815-.806C.635 6.045.431 5.298.746 4.623a7.92 7.92 0 0 1 .704-1.217c.428-.61 1.176-.807 1.82-.63l1.102.302c.067.019.177.011.3-.071.214-.143.437-.272.668-.386.133-.066.194-.158.211-.224l.29-1.106C6.717.645 7.264.095 8.007.031 8.23.01 8.394 0 8 0ZM7 4a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z"/>
      </svg>
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    height: 48px;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .logo {
    font-weight: 600;
    font-size: 15px;
  }

  .divider {
    color: var(--text-muted);
  }

  .repo-path {
    color: var(--text-secondary);
    font-size: 14px;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .branch-wrap {
    position: relative;
  }

  .branch {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .branch:hover,
  .branch.open {
    color: var(--text-primary);
    border-color: var(--accent-blue);
  }

  .caret {
    opacity: 0.6;
  }

  .branch-backdrop {
    position: fixed;
    inset: 0;
    z-index: 49;
  }

  .branch-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 50;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.4);
    min-width: 160px;
    max-height: 280px;
    overflow-y: auto;
    padding: 4px;
  }

  .branch-option {
    display: block;
    width: 100%;
    padding: 5px 10px;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
  }

  .branch-option:hover {
    background: var(--bg-tertiary);
  }

  .branch-option.current {
    color: var(--accent-blue);
    font-weight: 600;
  }

  .branch-option.remote {
    color: var(--text-secondary);
    opacity: 0.85;
  }

  .fetch-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    padding: 5px 10px;
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border-color);
    border-radius: 0;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    text-align: left;
    margin-bottom: 2px;
  }

  .fetch-btn:hover:not(:disabled) {
    color: var(--accent-blue);
    background: var(--bg-tertiary);
  }

  .fetch-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .branch-group-label {
    padding: 4px 10px 2px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .right {
    display: flex;
    gap: 8px;
  }

  .icon-btn {
    padding: 6px;
    background: transparent;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }
</style>
