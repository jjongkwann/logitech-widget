# logitech-widget

Windows desktop widget showing per-device battery levels of Logitech devices (like G HUB / Options+ shows), rendered as a frameless overlay pinned to the desktop.

## Stack

- **Tauri v2** — Rust backend + Web UI frontend, npm tooling
- **Rust** data layer: `hidapi` crate for direct HID++ communication (primary), `tokio-tungstenite` (or similar) for G HUB WebSocket fallback
- Frontend: plain HTML/CSS/TS (no heavy framework unless one is added deliberately)

## Commands

```
npm run tauri dev      # run in dev mode
npm run tauri build    # production build (single installer/exe)
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit tests
```

Prerequisites: Rust (stable-msvc), Node.js LTS, VS C++ Build Tools, WebView2 runtime (preinstalled on Win11).

## Architecture

```
src-tauri/src/
  battery/
    mod.rs        # BatterySource trait + DeviceBattery struct (shared model)
    hidpp/        # PRIMARY source: direct HID++ over hidapi
    ghub.rs       # FALLBACK source: G HUB local WebSocket (ws://localhost:9010)
  poller.rs       # polling loop; emits "battery-update" events to the webview
  tray.rs         # system tray icon + menu
  window.rs       # overlay window setup, pin-to-desktop (survive Win+D)
src/              # frontend (widget UI)
docs/PLAN.md      # roadmap, phase-by-phase verification criteria
```

Data flow: Rust-side polling task → `app_handle.emit("battery-update", payload)` → frontend `listen()`. Do NOT poll from the frontend via `invoke` in a `setInterval`; keep device/connection state in Rust.

**Source selection policy:** try HID++ first. Only fall back to G HUB WS for devices HID++ can't reach, or when explicitly configured. The shared model is `DeviceBattery { id, name, device_type, percentage, charging, source }` — both sources must normalize to it.

## Domain skills — read BEFORE touching the corresponding code

- `.claude/skills/hidpp-battery/SKILL.md` — HID++ protocol: report formats, feature discovery, battery features 0x1000/0x1001/0x1004, HID++ 1.0 registers, Windows/hidapi enumeration quirks, sleep handling. Read before any change under `battery/hidpp/`.
- `.claude/skills/ghub-websocket/SKILL.md` — G HUB local WS API (unofficial): endpoint, subprotocol, message envelope, paths, response shapes. Read before any change to `battery/ghub.rs`.
- `.claude/skills/tauri-widget/SKILL.md` — overlay window config, pin-to-desktop technique, tray, autostart, event patterns. Read before window/tray/startup changes.

## Hard-won constraints (do not rediscover these the hard way)

- On Windows, you MUST open the **vendor-defined HID collection** (usage page `0xFF00`/`0xFF43`), not the keyboard/mouse collections — the OS opens those exclusively. Filter devices by VID `0x046D` + vendor usage page; do NOT maintain a PID whitelist.
- A receiver exposes separate short (7-byte) and long (20-byte) report device paths — open both and route messages by length. Bluetooth-direct devices have long reports only.
- Sleeping wireless devices don't answer; the receiver replies with HID++ 1.0 error `0x8F`. Every request needs a timeout AND an `0x8F` handler. Treat "no answer" as offline, never as a bug to retry in a tight loop.
- The G HUB WS API is unofficial and version-fragile — isolate it behind the `BatterySource` trait so breakage never leaks past `ghub.rs`.
- Reference implementations when in doubt: Solaar (`pwr-Solaar/Solaar`, Python), LGSTrayBattery (`andyvorld/LGSTrayBattery`, C#), Linux kernel `hid-logitech-hidpp.c`. Prefer matching their behavior over the (sometimes wrong) spec drafts.

## Working agreements

- Each phase in `docs/PLAN.md` has explicit verification criteria — a phase is done only when its check passes on real hardware (or the documented substitute).
- HID++ parsing logic must be plain functions over byte slices (no I/O), unit-tested with captured fixture bytes. I/O lives at the edges.
- Korean is fine for user-facing docs and commit messages; code identifiers and comments in English.
