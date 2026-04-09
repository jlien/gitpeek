// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Repository, StatusOptions, DiffOptions, DiffFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager, State};
use base64::{Engine as _, engine::general_purpose};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

struct AppState {
    repo_path: Mutex<Option<PathBuf>>,
    run_inputs: Mutex<HashMap<u32, tokio::sync::mpsc::UnboundedSender<String>>>,
}

#[derive(Serialize, Clone)]
struct AssistantOutputEvent {
    run_id: u32,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct RepoInfo {
    path: String,
    branch: String,
    remote: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FileChange {
    path: String,
    status: String,
    staged: bool,
    additions: usize,
    deletions: usize,
}

/// Reject paths that are absolute or contain `..` components.
/// File paths from the frontend should always be relative git paths.
fn validate_relative_path(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return Err("Invalid path: must be relative".to_string());
    }
    if p.components().any(|c| c == std::path::Component::ParentDir) {
        return Err("Invalid path: must not contain '..'".to_string());
    }
    Ok(())
}

fn get_repo(state: &State<AppState>, path: Option<&str>) -> Result<Repository, String> {
    let repo_path = if let Some(p) = path {
        PathBuf::from(p)
    } else if let Some(p) = state.repo_path.lock().unwrap().clone() {
        p
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?
    };

    Repository::discover(&repo_path).map_err(|e| format!("Not a git repository: {}", e))
}

#[tauri::command]
fn get_repo_info(app: tauri::AppHandle, state: State<AppState>, path: Option<String>) -> Result<RepoInfo, String> {
    let repo = get_repo(&state, path.as_deref())?;

    // Update stored path and persist for next launch
    let workdir = repo.workdir().ok_or("No working directory")?;
    *state.repo_path.lock().unwrap() = Some(workdir.to_path_buf());
    let workdir_str = workdir.to_string_lossy().to_string();
    if let Ok(dir) = gitpeek_dir() {
        let _ = std::fs::write(dir.join("last_repo"), workdir_str.as_bytes());
    }
    push_recent_repo(&workdir_str);
    if let Ok(menu) = build_app_menu(&app) {
        let _ = app.set_menu(menu);
    }

    let head = repo.head().map_err(|e| e.to_string())?;
    let branch = head.shorthand().unwrap_or("HEAD").to_string();

    let remote = repo.find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|u| u.to_string()));

    Ok(RepoInfo {
        path: workdir.to_string_lossy().to_string(),
        branch,
        remote,
    })
}

#[tauri::command]
fn get_changed_files(state: State<AppState>) -> Result<Vec<FileChange>, String> {
    let repo = get_repo(&state, None)?;
    
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;
    let mut files = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let status = entry.status();

        // Determine file status
        let (status_str, staged) = if status.is_index_new() {
            ("added", true)
        } else if status.is_index_modified() {
            ("modified", true)
        } else if status.is_index_deleted() {
            ("deleted", true)
        } else if status.is_index_renamed() {
            ("renamed", true)
        } else if status.is_wt_new() {
            ("added", false)
        } else if status.is_wt_modified() {
            ("modified", false)
        } else if status.is_wt_deleted() {
            ("deleted", false)
        } else if status.is_wt_renamed() {
            ("renamed", false)
        } else {
            continue;
        };

        // Get diff stats (simplified - could be more accurate)
        let (additions, deletions) = get_file_stats(&repo, &path, staged).unwrap_or((0, 0));

        files.push(FileChange {
            path,
            status: status_str.to_string(),
            staged,
            additions,
            deletions,
        });
    }

    Ok(files)
}

fn get_file_stats(repo: &Repository, path: &str, staged: bool) -> Result<(usize, usize), git2::Error> {
    let mut opts = DiffOptions::new();
    opts.pathspec(path);

    let diff = if staged {
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))?
    };

    let stats = diff.stats()?;
    Ok((stats.insertions(), stats.deletions()))
}

#[tauri::command]
fn get_file_diff(state: State<AppState>, path: String) -> Result<String, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;

    let mut opts = DiffOptions::new();
    opts.pathspec(&path);

    // Try staged diff first. Pass None tree when HEAD doesn't exist (new repo) so
    // newly staged files are still shown as all-additions.
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let staged_diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
        .map_err(|e| e.to_string())?;

    let diff = if staged_diff.deltas().count() > 0 {
        staged_diff
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts))
            .map_err(|e| e.to_string())?
    };

    let mut diff_text = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            '>' => ">",
            '<' => "<",
            'F' => "",
            'H' => "@",
            _ => "",
        };
        diff_text.push_str(prefix);
        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }
        true
    }).map_err(|e| e.to_string())?;

    // Fallback for untracked files: read the file directly and format as a diff
    if diff_text.is_empty() {
        let workdir = repo.workdir().ok_or("No working directory")?;
        let file_path = workdir.join(&path);
        if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            let line_count = content.lines().count();
            diff_text.push_str(&format!(
                "--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n",
                path, line_count
            ));
            for line in content.lines() {
                diff_text.push('+');
                diff_text.push_str(line);
                diff_text.push('\n');
            }
        }
    }

    Ok(diff_text)
}

#[tauri::command]
fn stage_file(state: State<AppState>, path: String) -> Result<(), String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index.add_path(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unstage_file(state: State<AppState>, path: String) -> Result<(), String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let head = repo.head().map_err(|e| e.to_string())?;
    let obj = head.peel_to_commit().map_err(|e| e.to_string())?;
    
    repo.reset_default(Some(obj.as_object()), &[std::path::Path::new(&path)])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Branch diff ───────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct BranchFile {
    path: String,
    status: String,
}

/// Diff from the merge-base of `base` and `head` to `head` (equivalent to
/// `git diff base...head`). Accepts branch short-names or any revspec.
fn branch_diff_impl<'r>(
    repo: &'r Repository,
    base: &str,
    head: &str,
    opts: Option<&mut DiffOptions>,
) -> Result<git2::Diff<'r>, String> {
    let base_commit = repo
        .revparse_single(base)
        .map_err(|e| format!("Cannot find '{}': {}", base, e))?
        .peel_to_commit()
        .map_err(|e| format!("Cannot resolve '{}' to commit: {}", base, e))?;
    let head_commit = repo
        .revparse_single(head)
        .map_err(|e| format!("Cannot find '{}': {}", head, e))?
        .peel_to_commit()
        .map_err(|e| format!("Cannot resolve '{}' to commit: {}", head, e))?;

    let merge_base_oid = repo
        .merge_base(base_commit.id(), head_commit.id())
        .map_err(|e| format!("Cannot find merge base: {}", e))?;
    let merge_base_tree = repo
        .find_commit(merge_base_oid)
        .and_then(|c| c.tree())
        .map_err(|e| e.to_string())?;
    let head_tree = head_commit.tree().map_err(|e| e.to_string())?;

    repo.diff_tree_to_tree(Some(&merge_base_tree), Some(&head_tree), opts)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_branch_list(state: State<AppState>) -> Result<Vec<String>, String> {
    let repo = get_repo(&state, None)?;
    let mut branches = Vec::new();
    for branch in repo.branches(Some(git2::BranchType::Local)).map_err(|e| e.to_string())? {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        if let Some(name) = branch.name().map_err(|e| e.to_string())? {
            branches.push(name.to_string());
        }
    }
    Ok(branches)
}

#[tauri::command]
fn checkout_branch(state: State<AppState>, branch: String) -> Result<(), String> {
    let repo = get_repo(&state, None)?;
    let obj = repo
        .revparse_single(&format!("refs/heads/{}", branch))
        .map_err(|e| format!("Branch '{}' not found: {}", branch, e))?;
    repo.checkout_tree(&obj, None).map_err(|e| e.to_string())?;
    repo.set_head(&format!("refs/heads/{}", branch))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_branch_diff_files(state: State<AppState>, base: String, head: String) -> Result<Vec<BranchFile>, String> {
    let repo = get_repo(&state, None)?;
    let diff = branch_diff_impl(&repo, &base, &head, None)?;
    let mut files = Vec::new();
    for delta in diff.deltas() {
        let path = delta.new_file().path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let status = match delta.status() {
            git2::Delta::Added   => "added",
            git2::Delta::Deleted => "deleted",
            git2::Delta::Renamed => "renamed",
            _                    => "modified",
        };
        files.push(BranchFile { path, status: status.to_string() });
    }
    Ok(files)
}

#[tauri::command]
fn get_branch_file_diff(state: State<AppState>, base: String, head: String, path: String) -> Result<String, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&path);
    let diff = branch_diff_impl(&repo, &base, &head, Some(&mut opts))?;

    let mut diff_text = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            '>' => ">",
            '<' => "<",
            'F' => "",
            'H' => "@",
            _ => "",
        };
        diff_text.push_str(prefix);
        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }
        true
    }).map_err(|e| e.to_string())?;

    Ok(diff_text)
}

// ── Commit log ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct CommitInfo {
    hash: String,
    short_hash: String,
    message: String,
    author: String,
    time: i64,
}

#[derive(Serialize, Deserialize)]
struct CommitFile {
    path: String,
    status: String,
}

#[tauri::command]
fn get_commits(state: State<AppState>, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let repo = get_repo(&state, None)?;
    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;
    revwalk.push_head().map_err(|e| e.to_string())?;
    revwalk.set_sorting(git2::Sort::TIME).map_err(|e| e.to_string())?;

    let mut commits = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
        let hash = oid.to_string();
        let short_hash = hash[..7].to_string();
        commits.push(CommitInfo {
            short_hash,
            hash,
            message: commit.summary().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("").to_string(),
            time: commit.time().seconds(),
        });
    }
    Ok(commits)
}

#[tauri::command]
fn get_commit_files(state: State<AppState>, hash: String) -> Result<Vec<CommitFile>, String> {
    let repo = get_repo(&state, None)?;
    let oid = git2::Oid::from_str(&hash).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    let tree = commit.tree().map_err(|e| e.to_string())?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(|e| e.to_string())?;

    let mut files = Vec::new();
    diff.foreach(&mut |delta, _| {
        let path = delta.new_file().path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let status = match delta.status() {
            git2::Delta::Added => "added",
            git2::Delta::Deleted => "deleted",
            git2::Delta::Renamed => "renamed",
            _ => "modified",
        };
        files.push(CommitFile { path, status: status.to_string() });
        true
    }, None, None, None).map_err(|e| e.to_string())?;

    Ok(files)
}

#[tauri::command]
fn get_commit_file_diff(state: State<AppState>, hash: String, path: String) -> Result<String, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let oid = git2::Oid::from_str(&hash).map_err(|e| e.to_string())?;
    let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;
    let tree = commit.tree().map_err(|e| e.to_string())?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = DiffOptions::new();
    opts.pathspec(&path);

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))
        .map_err(|e| e.to_string())?;

    let mut diff_text = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            '>' => ">",
            '<' => "<",
            'F' => "",
            'H' => "@",
            _ => "",
        };
        diff_text.push_str(prefix);
        if let Ok(content) = std::str::from_utf8(line.content()) {
            diff_text.push_str(content);
        }
        true
    }).map_err(|e| e.to_string())?;

    Ok(diff_text)
}

// ── File line reader (for hunk expansion) ────────────────────────────────────

#[tauri::command]
fn get_file_lines(state: State<AppState>, path: String, start: usize, end: usize) -> Result<Vec<String>, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let workdir = repo.workdir().ok_or("No working directory")?;
    let file_path = workdir.join(&path);
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let lines: Vec<String> = content.lines()
        .enumerate()
        .filter(|(i, _)| *i + 1 >= start && *i + 1 <= end)
        .map(|(_, l)| l.to_string())
        .collect();
    Ok(lines)
}

#[tauri::command]
fn read_file(state: State<AppState>, path: String) -> Result<String, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let workdir = repo.workdir().ok_or("No working directory")?;
    let file_path = workdir.join(&path);
    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
fn read_file_base64(state: State<AppState>, path: String) -> Result<String, String> {
    validate_relative_path(&path)?;
    let repo = get_repo(&state, None)?;
    let workdir = repo.workdir().ok_or("No working directory")?;
    let bytes = std::fs::read(workdir.join(&path))
        .map_err(|e| format!("Failed to read file: {}", e))?;
    Ok(general_purpose::STANDARD.encode(&bytes))
}

// ── Commit ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn commit_staged(state: State<AppState>, message: String) -> Result<String, String> {
    let msg = message.trim().to_string();
    if msg.is_empty() {
        return Err("Commit message cannot be empty".to_string());
    }
    let repo = get_repo(&state, None)?;

    // Check there is something staged
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let staged_diff = repo
        .diff_tree_to_index(head_tree.as_ref(), None, None)
        .map_err(|e| e.to_string())?;
    if staged_diff.deltas().count() == 0 {
        return Err("Nothing staged to commit".to_string());
    }

    let mut index = repo.index().map_err(|e| e.to_string())?;
    index.read(false).map_err(|e| e.to_string())?;
    let tree_oid = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_oid).map_err(|e| e.to_string())?;
    let sig = repo.signature().map_err(|e| e.to_string())?;

    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let oid = if let Some(ref parent) = head_commit {
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[parent])
    } else {
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[])
    }
    .map_err(|e| e.to_string())?;

    Ok(oid.to_string()[..7].to_string())
}

// ── Assistant config ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct AssistantConfig {
    assistant_type: String,
    command: String,
    prompt_flag: String,
    #[serde(default)]
    extra_args: String,
}

fn gitpeek_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let dir = PathBuf::from(home).join(".config").join("gitpeek");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn config_path() -> Result<PathBuf, String> {
    Ok(gitpeek_dir()?.join("config.json"))
}

#[tauri::command]
fn get_last_repo() -> Option<String> {
    let path = gitpeek_dir().ok()?.join("last_repo");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[tauri::command]
fn get_assistant_config() -> Result<Option<AssistantConfig>, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(Some(serde_json::from_str(&json).map_err(|e| e.to_string())?))
}

#[tauri::command]
fn save_assistant_config(config: AssistantConfig) -> Result<(), String> {
    let path = config_path()?;
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_assistant(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    run_id: u32,
    prompt: String,
    file_path: String,
    line: usize,
    diff_context: String,
) -> Result<(), String> {
    let config = get_assistant_config()?
        .ok_or("No assistant configured. Open settings (⚙) to configure.")?;
    let repo_path = state
        .repo_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No repo loaded")?;

    let full_prompt = if file_path.is_empty() {
        prompt.clone()
    } else {
        format!(
            "In file `{}` at line {}, make the following change: {}\n\nCurrent diff context:\n{}",
            file_path, line, prompt, diff_context
        )
    };

    let mut args: Vec<String> = config.extra_args
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    // If a prompt flag is configured (e.g. -p), pass the prompt as a CLI arg
    // (headless single-turn mode). Otherwise the prompt is sent via stdin so
    // the process can stay alive for follow-up messages.
    let use_stdin_prompt = config.prompt_flag.is_empty();
    if !use_stdin_prompt {
        args.push(config.prompt_flag.clone());
        args.push(full_prompt.clone());
    }

    let mut cmd = tokio::process::Command::new(&config.command);
    for arg in &args {
        cmd.arg(arg);
    }

    use std::process::Stdio;
    let stdin_cfg = if use_stdin_prompt { Stdio::piped() } else { Stdio::null() };
    let mut child = cmd
        .current_dir(&repo_path)
        .env_remove("CLAUDECODE")
        .stdin(stdin_cfg)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start `{}`: {}", config.command, e))?;

    // In interactive (no -p) mode: keep stdin open for follow-up messages.
    if use_stdin_prompt {
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        state.run_inputs.lock().unwrap().insert(run_id, stdin_tx.clone());
        let _ = stdin_tx.send(full_prompt);
        let mut child_stdin = child.stdin.take().unwrap();
        tokio::spawn(async move {
            while let Some(text) = stdin_rx.recv().await {
                let _ = child_stdin.write_all(text.as_bytes()).await;
                let _ = child_stdin.write_all(b"\n").await;
                let _ = child_stdin.flush().await;
            }
        });
    }

    // Stream stdout line-by-line
    let stdout = child.stdout.take().unwrap();
    let app2 = app.clone();
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            app2.emit("assistant-output", AssistantOutputEvent { run_id, text: line }).ok();
        }
    });

    // Stream stderr line-by-line
    let stderr = child.stderr.take().unwrap();
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            app.emit("assistant-output", AssistantOutputEvent { run_id, text: line }).ok();
        }
    });

    let status = child.wait().await.map_err(|e| e.to_string())?;
    state.run_inputs.lock().unwrap().remove(&run_id);

    if status.success() {
        Ok(())
    } else {
        Err(format!("Process exited with {}", status))
    }
}

#[tauri::command]
fn send_to_assistant(state: State<AppState>, run_id: u32, text: String) -> Result<(), String> {
    let inputs = state.run_inputs.lock().unwrap();
    inputs
        .get(&run_id)
        .ok_or_else(|| "No active run".to_string())?
        .send(text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_assistant(state: State<AppState>, run_id: u32) -> Result<(), String> {
    state.run_inputs.lock().unwrap().remove(&run_id);
    Ok(())
}

// ── Recent repos ──────────────────────────────────────────────────────────────

fn load_recent_repos() -> Vec<String> {
    let path = match gitpeek_dir().map(|d| d.join("recent_repos.json")) {
        Ok(p) => p,
        Err(_) => return vec![],
    };
    if !path.exists() { return vec![]; }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

fn save_recent_repos(repos: &[String]) {
    if let Ok(path) = gitpeek_dir().map(|d| d.join("recent_repos.json")) {
        if let Ok(json) = serde_json::to_string(repos) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn push_recent_repo(workdir: &str) {
    let mut repos = load_recent_repos();
    repos.retain(|r| r != workdir);
    repos.insert(0, workdir.to_string());
    repos.truncate(10);
    save_recent_repos(&repos);
}

// ── Native menu ───────────────────────────────────────────────────────────────

fn build_app_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let recent_repos = load_recent_repos();

    // Recent-items must outlive the builder
    let recent_items: Vec<MenuItem<tauri::Wry>> = if recent_repos.is_empty() {
        vec![MenuItem::new(app, "No Recent Items", false, None::<&str>)?]
    } else {
        recent_repos
            .iter()
            .map(|p| {
                let label = std::path::Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.clone());
                MenuItem::with_id(app, format!("open-recent:{p}"), label, true, None::<&str>)
            })
            .collect::<tauri::Result<Vec<_>>>()?
    };

    let mut recent_builder = SubmenuBuilder::new(app, "Open Recent");
    for item in &recent_items {
        recent_builder = recent_builder.item(item);
    }
    let open_recent = recent_builder.build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&MenuItem::with_id(app, "file-open", "Open…", true, Some("CmdOrCtrl+O"))?)
        .item(&open_recent)
        .separator()
        .close_window()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, "GitPeek")
            .about(None)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        let window_menu = SubmenuBuilder::new(app, "Window")
            .minimize()
            .separator()
            .close_window()
            .build()?;
        return MenuBuilder::new(app)
            .item(&app_menu)
            .item(&file_menu)
            .item(&edit_menu)
            .item(&window_menu)
            .build();
    }

    #[cfg(not(target_os = "macos"))]
    MenuBuilder::new(app).item(&file_menu).item(&edit_menu).build()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            repo_path: Mutex::new(None),
            run_inputs: Mutex::new(HashMap::new()),
        })
        .setup(|app| {
            let menu = build_app_menu(app.handle())?;
            app.set_menu(menu)?;
            app.on_menu_event(|app_handle, event| {
                let id = event.id().as_ref();
                if id == "file-open" {
                    app_handle.emit("menu-open", ()).ok();
                } else if let Some(path) = id.strip_prefix("open-recent:") {
                    app_handle.emit("menu-open-recent", path.to_string()).ok();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_repo_info,
            get_changed_files,
            get_file_diff,
            stage_file,
            unstage_file,
            get_branch_list,
            checkout_branch,
            get_branch_diff_files,
            get_branch_file_diff,
            get_commits,
            get_commit_files,
            get_commit_file_diff,
            get_file_lines,
            read_file,
            read_file_base64,
            commit_staged,
            get_assistant_config,
            save_assistant_config,
            run_assistant,
            send_to_assistant,
            stop_assistant,
            get_last_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
