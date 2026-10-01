use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Selection {
    line: u32,
    column: u32,
    anchor_line: u32,
    anchor_column: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Attention {
    file: String,
    selections: Vec<Selection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Context {
    app_id: Option<String>,
    pid: Option<u32>,
    attention: Option<Attention>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Request {
    Publish {
        pid: u32,
        attention: Attention,
    },
    Query,
}

#[derive(Debug, Serialize, Deserialize)]
struct NiriWindow {
    pid: Option<u32>,
    app_id: Option<String>,
    title: Option<String>,
}

type Cache = Arc<Mutex<HashMap<u32, Attention>>>;

fn find_niri_socket() -> Option<std::path::PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let entries = std::fs::read_dir(runtime_dir).ok()?;
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("niri.wayland-") && file_name.ends_with(".sock") {
                    return Some(path);
                }
            }
        }
    }
    None
}

async fn get_focused_window() -> Option<NiriWindow> {
    let mut cmd = tokio::process::Command::new("niri");
    cmd.args(&["msg", "--json", "focused-window"]);
    if std::env::var("NIRI_SOCKET").is_err() {
        if let Some(sock) = find_niri_socket() {
            cmd.env("NIRI_SOCKET", sock);
        }
    }
    let output = cmd.output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

fn prune_cache(cache: &mut HashMap<u32, Attention>) {
    cache.retain(|pid, _| {
        Path::new(&format!("/proc/{}", pid)).exists()
    });
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

async fn handle_client(mut stream: UnixStream, cache: Cache) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];

    loop {
        let n = stream.read(&mut temp).await?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..n]);
        if buffer.contains(&b'\n') {
            break;
        }
    }

    let request_str = String::from_utf8_lossy(&buffer);
    let request: Request = match serde_json::from_str(request_str.trim()) {
        Ok(req) => req,
        Err(e) => {
            let err_msg = serde_json::json!({ "error": format!("Invalid request: {}", e) });
            stream.write_all(err_msg.to_string().as_bytes()).await?;
            return Ok(());
        }
    };

    match request {
        Request::Publish { pid, attention } => {
            let mut lock = cache.lock().await;
            prune_cache(&mut lock);
            lock.insert(pid, attention);
            let response = serde_json::json!({ "status": "ok" });
            stream.write_all(response.to_string().as_bytes()).await?;
        }
        Request::Query => {
            let window = get_focused_window().await;
            let mut attention = None;

            if let Some(ref win) = window {
                let mut resolved_pid = win.pid;

                // Try to extract PID from window title (resolves XWayland proxy PID mismatch)
                if let Some(ref title) = win.title {
                    if let Some(extracted_pid) = extract_pid_from_title(title) {
                        resolved_pid = Some(extracted_pid);
                    }
                }

                if let Some(pid) = resolved_pid {
                    let mut lock = cache.lock().await;
                    prune_cache(&mut lock);
                    if let Some(att) = lock.get(&pid) {
                        attention = Some(att.clone());
                    }
                }
            }

            let context = Context {
                app_id: window.as_ref().and_then(|w| w.app_id.clone()),
                pid: window.as_ref().and_then(|w| w.pid),
                attention,
            };

            let response = serde_json::to_string(&context)?;
            stream.write_all(response.as_bytes()).await?;
        }
    }

    Ok(())
}

async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/run/user/1000".to_string());
    let socket_path = Path::new(&runtime_dir).join("current.sock");

    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    let listener = UnixListener::bind(&socket_path)?;
    let mut permissions = std::fs::metadata(&socket_path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o600);
        std::fs::set_permissions(&socket_path, permissions)?;
    }

    println!("Current daemon listening on {:?}", socket_path);

    let cache: Cache = Arc::new(Mutex::new(HashMap::new()));

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let cache_clone = cache.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, cache_clone).await {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}

async fn run_client(command: &str) -> Result<(), Box<dyn std::error::Error>> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| "/run/user/1000".to_string());
    let socket_path = Path::new(&runtime_dir).join("current.sock");

    let mut stream = UnixStream::connect(socket_path).await?;
    
    let query_req = serde_json::json!({ "type": "Query" });
    stream.write_all(format!("{}\n", query_req).as_bytes()).await?;

    let mut response_buf = String::new();
    stream.read_to_string(&mut response_buf).await?;

    let context: Context = serde_json::from_str(&response_buf)?;
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home/tau".to_string());

    match command {
        "attention" => {
            if let Some(att) = context.attention {
                if let Some(sel) = att.selections.first() {
                    println!("{}:{}:{}", att.file, sel.line, sel.column);
                    return Ok(());
                }
            }
            std::process::exit(1);
        }
        "location" => {
            if let Some(att) = context.attention {
                if let Some(path) = Path::new(&att.file).parent() {
                    if let Some(path_str) = path.to_str() {
                        println!("{}", path_str);
                        return Ok(());
                    }
                }
            }
            println!("{}", home_dir);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            std::process::exit(1);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let command = &args[1];
        if command == "--daemon" {
            run_daemon().await?;
        } else {
            run_client(command).await?;
        }
    } else {
        eprintln!("Usage: current [--daemon | location | attention]");
        std::process::exit(1);
    }

    Ok(())
}
