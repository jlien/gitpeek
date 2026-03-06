// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use git2::{Repository, StatusOptions, DiffOptions, DiffFormat};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    repo_path: Mutex<Option<PathBuf>>,
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
fn get_repo_info(state: State<AppState>, path: Option<String>) -> Result<RepoInfo, String> {
    let repo = get_repo(&state, path.as_deref())?;
    
    // Update stored path and persist for next launch
    let workdir = repo.workdir().ok_or("No working directory")?;
    *state.repo_path.lock().unwrap() = Some(workdir.to_path_buf());
    if let Ok(dir) = gitpeek_dir() {
        let _ = std::fs::write(dir.join("last_repo"), workdir.to_string_lossy().as_bytes());
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
        let head = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head), None, Some(&mut opts))?
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

    // Try staged diff first, then working directory
    let diff = {
        let head = repo.head().and_then(|h| h.peel_to_tree());
        if let Ok(tree) = head {
            let staged_diff = repo.diff_tree_to_index(Some(&tree), None, Some(&mut opts));
            if let Ok(d) = staged_diff {
                if d.deltas().count() > 0 {
                    d
                } else {
                    repo.diff_index_to_workdir(None, Some(&mut opts))
                        .map_err(|e| e.to_string())?
                }
            } else {
                repo.diff_index_to_workdir(None, Some(&mut opts))
                    .map_err(|e| e.to_string())?
            }
        } else {
            repo.diff_index_to_workdir(None, Some(&mut opts))
                .map_err(|e| e.to_string())?
        }
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
    state: State<'_, AppState>,
    prompt: String,
    file_path: String,
    line: usize,
    diff_context: String,
) -> Result<String, String> {
    let config = get_assistant_config()?
        .ok_or("No assistant configured. Open settings (⚙) to configure.")?;
    let repo_path = state
        .repo_path
        .lock()
        .unwrap()
        .clone()
        .ok_or("No repo loaded")?;

    let full_prompt = format!(
        "In file `{}` at line {}, make the following change: {}\n\nCurrent diff context:\n{}",
        file_path, line, prompt, diff_context
    );

    // Build args list for display and execution
    let mut args: Vec<String> = config.extra_args
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if !config.prompt_flag.is_empty() {
        args.push(config.prompt_flag.clone());
    }
    args.push(full_prompt.clone());

    let repo_name = repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let cmd_display = format!(
        "$ {} {}\ncwd: {}",
        config.command,
        args.iter()
            .map(|a| if a.contains(' ') { format!("\"{}\"", a) } else { a.clone() })
            .collect::<Vec<_>>()
            .join(" "),
        repo_name
    );

    let mut cmd = tokio::process::Command::new(&config.command);
    for arg in &args {
        cmd.arg(arg);
    }
    let output = cmd
        .current_dir(&repo_path)
        .output()
        .await
        .map_err(|e| format!("{}\n\nFailed to run `{}`: {}", cmd_display, config.command, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut log = cmd_display.clone();
    log.push_str(&format!("\nexit: {}\n", output.status));
    if !stdout.is_empty() {
        log.push_str(&format!("\n[stdout]\n{}", stdout));
    }
    if !stderr.is_empty() {
        log.push_str(&format!("\n[stderr]\n{}", stderr));
    }

    if !output.status.success() {
        return Err(log);
    }

    Ok(log)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            repo_path: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_repo_info,
            get_changed_files,
            get_file_diff,
            stage_file,
            unstage_file,
            get_assistant_config,
            save_assistant_config,
            run_assistant,
            get_last_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
