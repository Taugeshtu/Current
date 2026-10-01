# Task: Untracked Window Attention Fallback

- [[Current]], [[task]], [[attention]], [[compositor]]

### Context
When a consumer queries `current attention` or `current location` on a focused window that has not published an attention entry into Current's cache (e.g., untracked apps, terminals without directory reporting, freshly spawned windows), Current currently falls back to `$HOME` for location or fails for attention.

### Future Work
- Explore heuristics or fallback providers:
  - Querying shell CWD from child process tree of terminal emulators (`foot`, `alacritty`).
  - Browser tab URL inspection (via MPRIS, D-Bus, or browser extension).
  - Compositor window title parsing (fallback URI extraction from title).
- Ensure fallbacks do not block or introduce latency into the hot query path.
