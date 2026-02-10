# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Dev Commands

```bash
bun install              # Install frontend dependencies (always use bun)
bun tauri dev            # Run dev server + Tauri window (hot-reload)
bun tauri build          # Production build (frontend + Rust)
bun run build            # Frontend-only build (tsc + vite)
```

Rust backend compiles automatically via Tauri CLI — no separate `cargo build` needed.

## Architecture

**Tauri 2 desktop app** — Rust backend + React frontend communicating via Tauri's `invoke` IPC.

### Backend (`src-tauri/src/`)

- **`lib.rs`** — Tauri app setup, registers all commands, locks window width
- **`commands.rs`** — All 6 `#[tauri::command]` handlers: `test_connection`, `scan_torrents`, `execute_replace`, `list_trackers`, `load_config`, `save_config`
- **`config.rs`** — `AppConfig` / `ConnectionConfig` / `Rule` structs, JSON persistence to `dirs::config_dir()/TrackerRelo/config.json`
- **`downloader/mod.rs`** — `DownloaderKind` enum dispatches to concrete clients; shared types (`TorrentInfo`, `TrackerInfo`, `ReplaceResult`)
- **`downloader/qbittorrent.rs`** — qBittorrent WebUI API v2 client (cookie-based auth via `reqwest`)
- **`downloader/transmission.rs`** — Transmission RPC client (session-id + Basic auth, auto-retry on 409)

### Frontend (`src/`)

- **`App.tsx`** — Single-file SPA. All UI state, Tauri `invoke` calls, and rendering in one component. Three card sections: Connection, Rules, Preview/Execute.
- **`components/ui/`** — shadcn/ui primitives (don't edit manually — regenerate with shadcn CLI)
- **`index.css`** — Tailwind v4 imports + CSS custom properties for light/dark theming

### Key patterns

- **No router** — single-page, single-component app
- **Config auto-save** — debounced (800ms) on any connection/rules change
- **Domain-based matching** — rules match via `tracker_url.contains(old_domain)`, not regex
- **Self-signed cert support** — `danger_accept_invalid_certs(true)` when HTTPS enabled
- **Release profile** — LTO + strip + `opt-level="s"` for small binaries

## CI

GitHub Actions (`.github/workflows/build.yml`) builds for macOS (aarch64 + x86_64), Ubuntu, Windows. Tag pushes (`v*`) create draft GitHub releases.
