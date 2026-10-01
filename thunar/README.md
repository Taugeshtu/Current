# Current — Thunar Hook

Integrates Thunar file manager with the `current` daemon to publish the active directory path of the focused window.

## How it works

1. **LD_PRELOAD Hook:** Intercepts `gtk_window_set_title` in GTK3 to identify `ThunarWindow` instances as they are initialized.
2. **Directory & Focus Signals:** Listens to `notify::current-directory`, `notify::is-active`, `focus-in-event`, and `map-event` on each window.
3. **URI Resolution:** Extracts the current directory object via GObject property and resolves its path using `thunarx_file_info_get_uri()` from `libthunarx-3.so`.
4. **Direct Socket Push:** Formats the location payload and writes non-blocking directly to `$XDG_RUNTIME_DIR/current.sock`.

## Usage

Preload `libthunar-current.so` when starting Thunar:

```bash
LD_PRELOAD=/path/to/libthunar-current.so thunar
```

The Current Nix flake builds and installs this library to `$out/lib/libthunar-current.so`.
