# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Install dependencies
npm install

# Run in development mode (starts Vite dev server + Tauri)
npm run tauri dev

# Build for production
npm run tauri build

# Frontend-only dev server (no Tauri, limited functionality)
npm run dev
```

There are no tests configured in this project.

### Linux dependencies (required)
```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

## Architecture

GitPeek is a desktop app built with **Tauri 2** (Rust backend + web frontend). The frontend uses **Svelte 4 + TypeScript + Vite**; the backend uses **git2** (libgit2 bindings) for all git operations.

### IPC: Frontend ↔ Backend

The frontend calls Rust functions via Tauri's `invoke()` from `@tauri-apps/api/core`. All backend commands are registered in `src-tauri/src/main.rs`:

| Command | Description |
|---|---|
| `get_repo_info` | Returns branch, path, remote URL; sets repo path in `AppState` |
| `get_changed_files` | Lists staged/unstaged file changes with +/- counts |
| `get_file_diff` | Returns raw diff text (staged diff preferred, falls back to workdir) |
| `stage_file` | Stages a file via git index |
| `unstage_file` | Unstages via `reset_default` to HEAD |

`AppState` holds a `Mutex<Option<PathBuf>>` for the current repo path, shared across command invocations. The repo is auto-discovered from `cwd` on startup, or set via folder picker.

### Frontend components

- **`App.svelte`** — root component; owns all state (`repoInfo`, `files`, `selectedFile`, `diff`, `viewMode`) and orchestrates `invoke()` calls
- **`Header.svelte`** — top bar with repo info, refresh, and open-folder actions
- **`FileTree.svelte`** — sidebar listing changed files with stage/unstage buttons; emits `select`, `stage`, `unstage` events
- **`DiffViewer.svelte`** — renders unified or split diff; parses raw diff text from the backend into typed `DiffLine`/`DiffHunk` structures client-side

### Styling

All CSS variables (colors, fonts) are defined in `src/app.css` following a GitHub dark theme. Components use `var(--bg-primary)`, `var(--diff-add-bg)`, etc. rather than hardcoded values.
