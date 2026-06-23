# AGENTS.md — wifi

Operating manual for the **`wifi`** crate: cross-platform Wi-Fi station control through
**OS-native APIs**, never by scraping `netsh`/`nmcli`/`iw` output. Built to replace the
CLI-driven `tools/wifi-rs`.

## What wifi is
A small Rust library that lets a host program **scan, connect, disconnect, query status, and
manage saved networks** on the machine's Wi-Fi adapter. Each OS is driven through its real
programming interface. Consumers in this workspace: **qjay** (the player appliance — Windows +
Linux, macOS later) and **qshell** (Windows-only). Product detail:
[docs/PROJECT_OVERVIEW.md](docs/PROJECT_OVERVIEW.md).

## Project architecture (source of truth)
A library crate (`src/lib.rs`). The public surface is `WiFi` + the value types; everything else
is internal. Layering, top → bottom:

- `wifi.rs` — **`WiFi`**, the public facade callers construct. Owns the compile-time-selected
  platform backend and forwards each async call. The only entry point.
- `interface.rs` — **`WifiBackend`** trait: the contract every platform implements. Core ops are
  required; profile/event ops have default `Unimplemented` bodies. No `dyn` — the facade binds one
  concrete backend per target, so native `async fn` in the trait is used.
- `platform/` — one backend per OS, selected by `#[cfg]` in `platform/mod.rs` and aliased to
  `Backend`:
  - `windows/` — **`WindowsWifi`**, the Native WiFi API (`wlanapi`) via the `windows` crate
    (`mod.rs` backend, `conv.rs` type mapping, `profile.rs` connect XML, `ipconfig.rs` IP-Helper
    details). The active target; all `WifiBackend` ops implemented.
    ([docs/AREAS/platform-windows.md](docs/AREAS/platform-windows.md))
  - `linux.rs` — **`LinuxWifi`**, NetworkManager over D-Bus via `zbus`. Planned (Phase 4).
    ([docs/AREAS/platform-linux.md](docs/AREAS/platform-linux.md))
  - `dummy.rs` — **`DummyWifi`**, compiles everywhere and returns `PlatformNotSupported`
    (macOS today).
- `types.rs` — backend-neutral vocabulary: `Network`, `Interface`, `Security`, `Credentials`,
  `ConnectRequest`, `ConnectionStatus`, `SavedNetwork`, `WifiEvent`, `WifiError`/`Result`.
  ([docs/AREAS/types.md](docs/AREAS/types.md))

Full layering + data flow: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Guardrails (read before changing anything)
- **OS-native only.** Never shell out to `netsh`, `nmcli`, `iw`, `wpa_cli`, etc. If a capability
  needs a CLI, it doesn't ship until the native API path exists. This is the crate's whole reason
  to exist (the rejected approach is `tools/wifi-rs`).
- **Backends never leak platform types.** A `windows`/`zbus` type must be mapped to a `types.rs`
  value before it crosses out of `platform/`. The public API is identical on every OS.
- **Standalone-buildable submodule.** `tools/wifi` is its own git repo composed into the q-lib
  workspace. Keep deps as explicit crates.io versions (no `workspace = true`); platform deps are
  `[target.'cfg(...)']`-gated so each OS builds only what it needs.
- **Default workspace build stays green.** The crate is a live member of the root workspace.
  `cargo check -p wifi` must pass on Windows.
- **Phased, honestly.** Unbuilt ops return `WifiError::Unimplemented(...)`, not fake data. What's
  real vs. planned lives in [docs/ROADMAP.md](docs/ROADMAP.md).
- **Self-contained code comments.** Comments explain the intended design, the invariants, and the
  constraints of the code they sit on, and stand entirely on their own. They are professional and
  technical — not narrative, not status/roadmap chatter ("Phase 1", "first target"), and they never
  point the reader at `docs/`, an ADR, or any external location. The docs hold the *why* across the
  crate; a comment holds the *what and the constraints* of its own code. (Rustdoc intra-doc links to
  other code items are fine — those are API references, not prose pointers.)

## Living documentation rules
- `docs/` is this crate's AI-maintained memory. Before a meaningful change, read
  [docs/AI_CONTEXT.md](docs/AI_CONTEXT.md) and the relevant [docs/AREAS/](docs/AREAS/README.md) file.
- **Git history is the task log.** Do not add a per-task changelog directory. Record durable
  knowledge in `docs/AREAS/*.md` and decisions in [docs/DECISIONS/](docs/DECISIONS/README.md) only.
- Update an area doc / ADR only when lasting structure, conventions, or decisions change. Don't
  restate code.
