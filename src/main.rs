mod focus;

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
    title: Option<String>,
    pid: Option<u32>,
    window_id: Option<u64>,
    attention: Option<Attention>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Request {
    Publish {
        #[serde(default)]
        pid: Option<u32>,
        attention: Attention,
    },
    Query,
}

type Cache = Arc<Mutex<HashMap<u64, Attention>>>;

async fn prune_cache(cache: &mut HashMap<u64, Attention>) {
    if let Some(active_ids) = focus::get_active_window_ids().await {
        cache.retain(|id, _| active_ids.contains(id));
    }
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
        Request::Publish { attention, .. } => {
            if let Some(focus) = focus::get_focus().await {
                let mut lock = cache.lock().await;
                prune_cache(&mut lock).await;
                lock.insert(focus.window_id, attention);
            }
            let response = serde_json::json!({ "status": "ok" });
            stream.write_all(response.to_string().as_bytes()).await?;
        }
        Request::Query => {
            let focus = focus::get_focus().await;
            let mut attention = None;

            if let Some(ref f) = focus {
                let mut lock = cache.lock().await;
                prune_cache(&mut lock).await;
                if let Some(att) = lock.get(&f.window_id) {
                    attention = Some(att.clone());
                }
            }

            let context = Context {
                app_id: focus.as_ref().and_then(|f| f.app_id.clone()),
                title: focus.as_ref().and_then(|f| f.title.clone()),
                pid: focus.as_ref().and_then(|f| f.pid),
                window_id: focus.as_ref().map(|f| f.window_id),
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
                } else {
                    println!("{}", att.file);
                }
                return Ok(());
            }
            std::process::exit(1);
        }
        "location" => {
            if let Some(att) = context.attention {
                let p = Path::new(&att.file);
                let loc = if p.is_dir() {
                    Some(p)
                } else {
                    p.parent()
                };
                if let Some(path_str) = loc.and_then(|p| p.to_str()) {
                    println!("{}", path_str);
                    return Ok(());
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
