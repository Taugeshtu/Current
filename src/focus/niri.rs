use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use super::FocusPoint;

#[derive(Debug, Deserialize)]
struct NiriWindow {
    id: Option<u64>,
    pid: Option<u32>,
    app_id: Option<String>,
    title: Option<String>,
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

pub async fn get_focus() -> Option<FocusPoint> {
    let output = prepare_niri_cmd("focused-window").output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let win: NiriWindow = serde_json::from_slice(&output.stdout).ok()?;
    let window_id = win.id?;
    Some(FocusPoint {
        window_id,
        pid: win.pid,
        app_id: win.app_id,
        title: win.title,
    })
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
