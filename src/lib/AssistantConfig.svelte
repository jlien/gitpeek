<script lang="ts">
  import { createEventDispatcher, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  const dispatch = createEventDispatcher();

  interface AssistantConfig {
    assistant_type: string;
    command: string;
    prompt_flag: string;
    extra_args: string;
  }

  const presets: Record<string, { command: string; prompt_flag: string; extra_args: string }> = {
    'claude-code': { command: 'claude', prompt_flag: '-p', extra_args: '--dangerously-skip-permissions' },
    'codex':       { command: 'codex',  prompt_flag: '',   extra_args: '' },
    'custom':      { command: '',       prompt_flag: '',   extra_args: '' },
  };

  let config: AssistantConfig = {
    assistant_type: 'claude-code',
    command: 'claude',
    prompt_flag: '-p',
    extra_args: '--dangerously-skip-permissions',
  };
  let saving = false;
  let saveError: string | null = null;

  onMount(async () => {
    try {
      const existing = await invoke<AssistantConfig | null>('get_assistant_config');
      if (existing) config = { extra_args: '', ...existing };
    } catch {}
  });

  function onTypeChange() {
    const preset = presets[config.assistant_type];
    if (preset) {
      config.command = preset.command;
      config.prompt_flag = preset.prompt_flag;
      config.extra_args = preset.extra_args;
    }
  }

  async function save() {
    saving = true;
    saveError = null;
    try {
      await invoke('save_assistant_config', { config });
      dispatch('close');
    } catch (e) {
      saveError = String(e);
    }
    saving = false;
  }

  $: invocationPreview = [
    config.command,
    config.extra_args,
    config.prompt_flag,
    '"<prompt>"',
  ].filter(Boolean).join(' ');
</script>

<div class="backdrop" on:click|self={() => dispatch('close')} role="none">
  <div class="modal">
    <div class="modal-header">
      <h2>Assistant Settings</h2>
      <button class="close-btn" on:click={() => dispatch('close')}>✕</button>
    </div>

    <div class="modal-body">
      <label>
        <span>Assistant</span>
        <select bind:value={config.assistant_type} on:change={onTypeChange}>
          <option value="claude-code">Claude Code</option>
          <option value="codex">Codex</option>
          <option value="custom">Custom</option>
        </select>
      </label>

      <label>
        <span>Command</span>
        <input type="text" bind:value={config.command} placeholder="e.g. claude" spellcheck="false" />
      </label>

      <label>
        <span>Prompt flag <span class="optional">(leave blank if prompt is positional)</span></span>
        <input type="text" bind:value={config.prompt_flag} placeholder="e.g. -p" spellcheck="false" />
      </label>

      <label>
        <span>Extra flags <span class="optional">(space-separated, added before the prompt)</span></span>
        <input type="text" bind:value={config.extra_args} placeholder="e.g. --allowedTools Edit,Write,Read,Bash" spellcheck="false" />
      </label>

      <p class="hint">Invocation: <code>{invocationPreview}</code></p>

      {#if saveError}
        <p class="error">{saveError}</p>
      {/if}
    </div>

    <div class="modal-footer">
      <button on:click={() => dispatch('close')}>Cancel</button>
      <button class="primary" on:click={save} disabled={saving || !config.command}>
        {saving ? 'Saving…' : 'Save'}
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    width: 440px;
    max-width: 90vw;
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 16px;
    padding: 4px;
    line-height: 1;
  }

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-secondary);
  }

  .optional {
    color: var(--text-muted);
    font-weight: normal;
  }

  select,
  input[type='text'] {
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-mono);
    padding: 8px 10px;
  }

  select:focus,
  input[type='text']:focus {
    outline: none;
    border-color: var(--accent-blue);
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .hint code {
    font-family: var(--font-mono);
    color: var(--text-secondary);
  }

  .error {
    margin: 0;
    font-size: 13px;
    color: var(--accent-red);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 20px;
    border-top: 1px solid var(--border-color);
  }

  .modal-footer button {
    padding: 6px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    border: 1px solid var(--border-color);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
  }

  .modal-footer button.primary {
    background: var(--accent-blue);
    border-color: var(--accent-blue);
    color: white;
  }

  .modal-footer button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
