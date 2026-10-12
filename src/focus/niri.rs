use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct NiriWindow {
    id: Option<u64>,
    pid: Option<u32>,
    title: Option<String>,
}

fn extract_pid_from_title(title: &str) -> Option<u32> {
    if let Some(start_idx) = title.rfind("[PID: ") {
        let remainder = &title[start_idx + 6..];
        if let Some(end_idx) = remainder.find(']') {
            let pid_str = &remainder[..end_idx];
            return pid_str.trim().parse::<u32>().ok();
        }
    }
    None
}

fn window_matches_pid(win: &NiriWindow, target_pid: u32) -> bool {
    if win.pid == Some(target_pid) {
        return true;
    }
    if let Some(ref title) = win.title {
        if extract_pid_from_title(title) == Some(target_pid) {
            return true;
        }
    }
    false
}

fn find_niri_socket() -> Option<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let entries = std::fs::read_dir(runtime_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with("niri.wayland-") && file_name.ends_with(".sock") {
                return Some(path);
            }
        }
    }
    None
}

fn prepare_niri_cmd(subcmd: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("niri");
    cmd.args(&["msg", "--json", subcmd]);
    if std::env::var("NIRI_SOCKET").is_err() {
        if let Some(sock) = find_niri_socket() {
            cmd.env("NIRI_SOCKET", sock);
        }
    }
    cmd
}

pub async fn get_focus_window_id() -> Option<u64> {
    let output = prepare_niri_cmd("focused-window").output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let win: NiriWindow = serde_json::from_slice(&output.stdout).ok()?;
    win.id
}

pub async fn get_active_window_ids() -> Option<HashSet<u64>> {
    let output = prepare_niri_cmd("windows").output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let windows: Vec<NiriWindow> = serde_json::from_slice(&output.stdout).ok()?;
    let ids = windows.into_iter().filter_map(|w| w.id).collect();
    Some(ids)
}

pub async fn find_window_id_for_pid(target_pid: u32) -> Option<u64> {
    if let Ok(output) = prepare_niri_cmd("focused-window").output().await {
        if output.status.success() {
            if let Ok(win) = serde_json::from_slice::<NiriWindow>(&output.stdout) {
                if window_matches_pid(&win, target_pid) {
                    return win.id;
                }
            }
        }
    }

    let output = prepare_niri_cmd("windows").output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let windows: Vec<NiriWindow> = serde_json::from_slice(&output.stdout).ok()?;
    for win in windows {
        if window_matches_pid(&win, target_pid) {
            return win.id;
        }
    }

    None
}
