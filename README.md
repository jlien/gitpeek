# GitPeek

A local PR diff viewer — preview your changes before pushing, with a GitHub-like interface and inline AI-assisted editing.

## Features

- File tree with staged/unstaged changes
- Side-by-side or unified diff view
- Stage/unstage individual files from the diff view
- Inline AI prompts: annotate any diff line to request a code change
- Remembers the last opened repository
- GitHub-style dark theme
- Fast — uses native git operations via libgit2

## Requirements

- **Rust** 1.77+ — [rustup.rs](https://rustup.rs)
- **Node.js** 18+

**Linux only** — install WebKit and GTK dependencies:
```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

## Installation

### Run from source

```bash
git clone https://github.com/jlien/gitpeek.git
cd gitpeek
npm install
npm run tauri dev
```

### Build a release binary

```bash
npm run tauri build
```

The compiled app will be in `src-tauri/target/release/bundle/`.

## Usage

1. Launch GitPeek — it auto-detects the git repo in the current working directory, or reopens the last used repo.
2. Click the **folder icon** in the header to open a different repository.
3. Click any file in the sidebar to view its diff.
4. Use **+** / **−** buttons on each file to stage or unstage it.
5. Toggle **Split** / **Unified** view in the diff header.

### AI-assisted editing

GitPeek can invoke a coding assistant (Claude Code, Codex, or any compatible CLI) to apply changes directly to your files based on inline prompts.

#### Setup

1. Click the **⚙ gear icon** in the header to open Assistant Settings.
2. Configure your assistant:

| Field | Description |
|---|---|
| **Assistant** | Preset for Claude Code, Codex, or Custom |
| **Command** | The CLI binary to run (e.g. `claude`, `codex`) |
| **Prompt flag** | Flag placed before the prompt argument (e.g. `-p`). Leave blank if the prompt is positional. |
| **Extra flags** | Additional flags passed before the prompt flag (e.g. permission/tool flags) |

3. Click **Save**.

#### Preset defaults

| Assistant | Command | Extra flags |
|---|---|---|
| Claude Code | `claude` | `--dangerously-skip-permissions` |
| Codex | `codex` | _(none)_ |

The full invocation is: `<command> [extra flags] [prompt flag] "<prompt>"`, run from the repository root.

#### Using inline prompts

1. Hover over any diff line — a comment bubble (💬) appears in the left gutter.
2. Click it to open an inline prompt input.
3. Type a description of the change you want (e.g. *"extract this into a helper function"*).
4. Press **Apply** or **⌘↵** / **Ctrl↵**.

GitPeek sends the assistant your prompt plus the surrounding diff context, runs the command in the repository root, then refreshes the diff automatically. The **Assistant Output** panel on the right shows the full command, exit code, and stdout/stderr for every run.

#### Custom assistant

Set **Assistant** to **Custom** and provide any command that accepts a prompt as a string argument. For example, a wrapper script:

```bash
#!/bin/bash
# my-ai.sh — writes changes to disk, then exits 0
my-ai-tool --apply "$1"
```

Configure Command as `./my-ai.sh` with no prompt flag (positional argument).

#### Configuration file

Settings are stored in `~/.config/gitpeek/config.json`:

```json
{
  "assistant_type": "claude-code",
  "command": "claude",
  "prompt_flag": "-p",
  "extra_args": "--dangerously-skip-permissions"
}
```

## Contributing

Contributions are welcome. Please open an issue before starting work on a large change so we can discuss the approach.

### Getting started

```bash
git clone https://github.com/jlien/gitpeek.git
cd gitpeek
npm install
```

Run in development mode (hot-reload for the frontend, Rust recompiles on change):

```bash
npm run tauri dev
```

### Project structure

```
src/                   Svelte frontend
  lib/
    Header.svelte      Top bar with repo info and actions
    FileTree.svelte    Sidebar file list with stage/unstage
    DiffViewer.svelte  Diff rendering (unified + split) with inline prompts
    AssistantConfig.svelte  Assistant settings modal
    AssistantOutput.svelte  Output panel for assistant runs
src-tauri/src/main.rs  Rust backend — all git operations and assistant invocation
```

All git operations go through [git2](https://github.com/rust-lang/git2-rs) (libgit2 bindings). The frontend communicates with the backend exclusively via Tauri's typed `invoke()` IPC — there are no direct filesystem or process calls from the frontend.

### Submitting changes

1. Fork the repository and create a branch from `main`.
2. Make your changes. Keep commits focused — one logical change per commit.
3. Test the dev build with `npm run tauri dev` and a release build with `npm run tauri build`.
4. Open a pull request against `main` with a clear description of what changed and why.

For bug reports, please include:
- GitPeek version (or commit hash)
- OS and distribution
- Steps to reproduce
- If using the AI assistant feature, the contents of the Assistant Output panel

## Tech stack

- **Frontend:** Svelte 4 + TypeScript + Vite
- **Backend:** Rust + Tauri 2 + git2 (libgit2)
- **Styling:** Custom CSS (GitHub dark theme)

## License

MIT — see [LICENSE](LICENSE).
