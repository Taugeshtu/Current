# Current — Thunar Hook

Integrates Thunar file manager with the `current` daemon to publish the active directory path of the focused window as folder attention.

## How it works

1. **LD_PRELOAD Hook & Cleanup:** Loaded via `LD_PRELOAD`. On initialization (`__attribute__((constructor))`), it unsets `LD_PRELOAD` from the environment to avoid leaking into child processes (such as GTK4 applications like Purse).
2. **Window Interception:** Intercepts `gtk_window_set_title` in GTK3 to identify `ThunarWindow` instances as they are created.
3. **Directory & Focus Signals:** Listens to `notify::current-directory`, `notify::is-active`, `focus-in-event`, and `map-event` on each window.
4. **URI Resolution:** Extracts the current directory object via GObject property and resolves its path using `thunarx_file_info_get_uri()` from `libthunarx-3.so`.
5. **Direct Socket Push:** Formats the folder attention payload (`{"type":"Publish","attention":{"folder": ...}}`) and writes non-blocking directly to `$XDG_RUNTIME_DIR/current.sock`.

## Usage

Preload `libthunar-current.so` when starting Thunar:

```bash
LD_PRELOAD=/path/to/libthunar-current.so thunar
```

The Current Nix flake builds and installs this library to `$out/lib/libthunar-current.so`.
