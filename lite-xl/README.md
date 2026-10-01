# Current — Lite-XL Plugin

Integrates Lite-XL with the `current` daemon to publish active document paths and selection coordinates.

## How it works
1. **Window Title PID:** Overrides `system.set_window_title` to append ` [PID: <actual_host_pid>]` to the window title. This allows the `current` daemon to resolve the host process PID even when running under an XWayland proxy (like `xwayland-satellite`).
2. **Focus/Movement Hooks:** Hooks `Doc:set_selections` and `core.set_active_view` to detect cursor movement and document changes.
3. **Throttled Updates:** Wakes up every 100ms using a background thread (`core.add_thread`) and pushes changes via `system.exec("sh -c 'echo ... | nc -U ...'")` asynchronously, preventing editor lag.

## Install
Copy `current.lua` into your Lite-XL plugins folder:

```bash
cp current.lua ~/.config/lite-xl/plugins/current.lua
```

Restart Lite-XL to load the plugin.
