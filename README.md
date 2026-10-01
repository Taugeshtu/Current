An on-demand context resolver for the Future ecosystem. It answers a simple question at any moment: *what is the user paying attention to right now?*

### why
Instead of every script or hotkey hacking together custom focused-window extraction logic, Current acts as the central context coordinator. Applications (like Lite-XL or VS Code) push their cursor caret positions here, and scripts query it at the point of need. 

### how
Current runs as a lightweight daemon listening on a Unix domain socket at `$XDG_RUNTIME_DIR/current.sock`. 
- **Publish (Push):** Editors send their current document focus and selection coordinates (JSON) over the socket. Current caches this by window PID.
- **Query (Pull):** When asked "what is context?", Current queries the `niri` compositor for the focused window PID, looks it up in the cache, and resolves the context.
- **Pruning:** Cache entries are pruned automatically using Linux `/proc/{pid}` to avoid leaking dead processes.

# Install

### getting the binary
Build and install to your user local path:
```bash
# inside the Current project folder
cargo install --path . --root ~/.local
```
This produces `~/.local/bin/current`.

### making it run automatically on your system
The best way to run Current is as a user systemd service.
Write the following into `~/.config/systemd/user/current.service`:

```ini
[Unit]
Description=Current Context Daemon
After=niri.service

[Service]
Type=simple
ExecStart=%h/.local/bin/current --daemon
Restart=on-failure
RestartSec=2
StartLimitIntervalSec=30
StartLimitBurst=5

[Install]
WantedBy=default.target
```

Enable and start the service:
```bash
systemctl --user enable current --now
```

---

# CLI Interface

Current is queried on demand via the command line:

### `current attention`
Prints the active file path, line, and column of the primary cursor:
```
/media/veracrypt1/_PROJECTS/_Future/Current/src/main.rs:12:4
```
If no editor is focused or no attention context has been registered, it outputs **nothing** to stdout and exits with status **`1`**.

### `current location`
Prints the parent directory of the active file:
```
/media/veracrypt1/_PROJECTS/_Future/Current/src
```
If no context exists, it falls back to `$HOME` (as "home is the floor") and exits with status **`0`**.

---

# Socket Protocol (JSON)

For programmatic access, connect to `$XDG_RUNTIME_DIR/current.sock`.

### Publishing Context (from Editors)
Send a single JSON line to the socket containing the window's PID and its active attention:
```json
{
  "type": "Publish",
  "pid": 550405,
  "attention": {
    "file": "/path/to/doc.md",
    "selections": [
      { "line": 15, "column": 20, "anchor_line": 15, "anchor_column": 8 }
    ]
  }
}
```

### Querying Context
Send the query JSON:
```json
{ "type": "Query" }
```
It returns the active focused window's context:
```json
{
  "app_id": "lite-xl",
  "pid": 550405,
  "attention": {
    "file": "/path/to/doc.md",
    "selections": [
      { "line": 15, "column": 20, "anchor_line": 15, "anchor_column": 8 }
    ]
  }
}
```
