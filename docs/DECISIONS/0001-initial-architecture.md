# Decision: Initial architecture — OS-native backends behind one async facade

## Status
Accepted 2026-06-23. Skeleton implemented the same day (Phase 0, see
[../ROADMAP.md](../ROADMAP.md)).

## Context
The workspace needs programmatic Wi-Fi control (scan/connect/disconnect/status, saved profiles,
events) for **qjay** (Windows + Linux, macOS later) and **qshell** (Windows). The existing
`tools/wifi-rs` does this by **shelling out to `netsh`/`nmcli`** and parsing text — brittle,
locale-sensitive, slow, and event-less. We want a clean replacement and had to settle, up front:
how OSes are driven, sync vs async, the Linux daemon, and how much to build now.

Decisions taken with the owner (2026-06-23):
- **Capabilities:** core station ops + profile management + connection events now; hotspot &
  Enterprise later.
- **Linux:** NetworkManager first, but leave room for `wpa_supplicant` behind the same trait.
- **API model:** async (tokio).
- **Priority:** Windows first (current dev OS); qjay + qshell are the consumers.

## Decision
1. **OS-native only.** Each platform is driven through its real API — Windows Native WiFi
   (`wlanapi`) via the `windows` crate; Linux NetworkManager over D-Bus via `zbus`. No process
   spawning of CLI tools, ever. This is the crate's reason to exist.
2. **One facade, one backend, compile-time selected.** Public `WiFi` (`wifi.rs`) owns a
   `platform::Backend` that `platform/mod.rs` aliases via `#[cfg]` to exactly one of
   `WindowsWifi` / `LinuxWifi` / `DummyWifi`. No `dyn`, no runtime selection — mirrors
   `tools/bluetooth`. This permits native `async fn` in the `WifiBackend` trait (no `async-trait`).
3. **Async (tokio) API.** Matches D-Bus's async idiom and the wait-for-notification nature of
   scan/connect, and makes the Phase-3 event stream (`tokio::sync::mpsc`) natural. Sync `wlanapi`
   calls are wrapped to keep the surface uniform.
4. **Sealed backends, neutral types.** Backends map OS structures onto `types.rs` before returning;
   no platform type crosses the public API, so callers are OS-agnostic.
5. **Trait with optional defaults + phased build.** Core ops are required; later ops
   (`saved_networks`/`forget`/`subscribe`) default to `Unimplemented` so backends advance
   independently. Unbuilt ops return `Unimplemented`/`PlatformNotSupported`, never fake data.
6. **Standalone submodule, workspace member.** `tools/wifi` is its own git repo with explicit
   crates.io deps (no `workspace = true`); platform deps are `cfg`-gated. It's a live member of the
   q-lib workspace so the default build covers it.

## Consequences
- **Enables:** robust, fast, event-capable Wi-Fi control; identical caller code on every OS; a
  second Linux daemon (`wpa_supplicant`) or a macOS (CoreWLAN) backend added as just another trait
  impl; per-OS lean builds via `cfg`-gated deps.
- **Constrains:** every capability must be expressible through each OS's native API (more upfront
  work than CLI scraping); adding a public type field is a breaking change for qjay/qshell;
  compile-time selection means no runtime backend swap (acceptable — OS is fixed at build).
- **Pairs with:** the phased [ROADMAP.md](../ROADMAP.md) (Windows core → profiles → events → Linux
  → advanced) and the `tools/bluetooth` (Windows-native, sync) / `tools/ble` (Linux D-Bus, async)
  precedents in the workspace.
- **Open:** multi-interface targeting (facade currently implies the primary interface); the exact
  `Credentials::Enterprise` shape; the NM-vs-wpa_supplicant runtime detection mechanism — all
  deferred to their phases.
