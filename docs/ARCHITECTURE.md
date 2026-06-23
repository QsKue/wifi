# Architecture

How `wifi` is put together. Module boundaries are the source of truth in [../AGENTS.md](../AGENTS.md);
this file explains the layering and data flow.

## Layers
```
caller (qjay / qshell / example)
        │  constructs, awaits
        ▼
WiFi  (wifi.rs)              ← public facade; owns one Backend; forwards calls
        │  delegates
        ▼
WifiBackend trait (interface.rs)   ← the contract: scan/connect/disconnect/status/… (async)
        │  implemented by exactly one, chosen at compile time
        ▼
platform::Backend  (platform/mod.rs aliases via #[cfg])
   ├─ WindowsWifi (platform/windows/)    → wlanapi  (windows crate)     [active]
   ├─ LinuxWifi   (platform/linux.rs)    → NetworkManager / D-Bus (zbus) [planned]
   └─ DummyWifi   (platform/dummy.rs)    → PlatformNotSupported          [fallback]
        │  maps OS structures → types.rs before returning
        ▼
types.rs   ← backend-neutral values crossing every boundary
```

## Key design choices
- **Compile-time backend selection, no `dyn`.** `platform/mod.rs` uses `#[cfg(windows)]` /
  `#[cfg(target_os = "linux")]` / else to alias one concrete struct to `Backend`. The facade holds
  it directly. This mirrors `tools/bluetooth`'s aliasing pattern and lets the trait use native
  `async fn` (no `async-trait`, no boxed futures).
- **Async-first.** Every backend op is `async`. D-Bus (Linux) is async by nature; the Windows
  `wlanapi` calls are synchronous but wrapped so the public API is uniform, and scan/connect
  (which wait on OS notifications) fit the async model cleanly. Events use
  `tokio::sync::mpsc`. ADR [0001](DECISIONS/0001-initial-architecture.md).
- **Backends are sealed.** A `windows::…` or `zbus::…` value is converted to a `types.rs` value
  *inside* the backend. Nothing platform-specific appears in the public API, so callers write the
  same code on every OS.
- **Capabilities are trait methods with optional defaults.** Core ops are required on the trait;
  later-phase ops (`saved_networks`, `forget`, `subscribe`) ship with default `Unimplemented`
  bodies so one backend can advance ahead of the others.

## Data flow — a scan
1. Caller: `wifi.scan().await`.
2. `WiFi::scan` → `Backend::scan` (the cfg-selected impl).
3. Backend calls the OS: Windows `WlanScan` + wait for completion + `WlanGetAvailableNetworkList`;
   Linux NM `RequestScan` + read `AccessPoints`.
4. Backend maps each OS record → `types::Network` (ssid, bssid, `Security`, normalized `signal`
   0–100, frequency, `saved`).
5. `Vec<Network>` returns up the stack unchanged.

## Cross-platform build
`tools/wifi` is a standalone git submodule wired into the q-lib workspace as a live member. Deps
are explicit crates.io versions; platform deps are `[target.'cfg(...)']`-gated, so a Windows build
pulls only the `windows` crate and a Linux build pulls only `zbus`. The `dummy` backend keeps the
crate compiling on any other target.

Per-module conventions and gotchas: [AREAS/](AREAS/README.md).
