# Roadmap

Phased plan. Status labels: **[done]**, **[in progress]**, **[planned]**, **[deferred]**.
Built behavior is verified by code; this file holds the *plan and ordering*.

## Phase 0 — Skeleton & contract — [done] (2026-06-23)
- Crate scaffold: `WiFi` facade, `WifiBackend` trait, `types.rs`, `#[cfg]` platform selection,
  `dummy` fallback, `scan` example. Compiles on Windows with no warnings; wired as a live workspace
  member. All ops return `Unimplemented`/`PlatformNotSupported`.

## Phase 1 — Windows core station ops — [done] (2026-06-23)
Native WiFi API (`wlanapi`) in `platform/windows/`. Primary target (current dev OS). Verified live
on an Intel Wi-Fi 7 adapter (`interfaces`/`scan`/`status`).
- `WlanOpenHandle` on construct, `WlanCloseHandle` on `Drop` (handle wrapped `Send`/`Sync`).
- `interfaces` → `WlanEnumInterfaces`.
- `scan` → `WlanScan`, await scan-complete via `WlanRegisterNotification`, `WlanGetAvailableNetworkList`
  → `Vec<Network>` (deduped by SSID, strongest signal kept).
- `connect` → `WlanSetProfile` (open + WPA2-PSK XML), `WlanConnect`. Initiates association and
  returns; caller polls `status`.
- `disconnect` → `WlanDisconnect`.
- `status` → `WlanQueryInterface(current_connection)`.
- `wlanSignalQuality` (0–100) and `dot11DefaultAuthAlgorithm` → `types::Security`.

**Verified live:** `interfaces`, `scan`, `status`, `disconnect`, and `connect(Saved)` (disconnect →
reconnect round-trip, no stored credentials altered).

`connect()` **awaits the real outcome** (it no longer returns on initiate): it registers for the
terminal connection notification on a dedicated handle, then returns `Ok` on success or
`AuthFailed` on a wrong key (MSMSEC reason band `0x40000..0x50000`; a wrong key takes ~25s as
Windows retries, so the timeout is 40s). The PSK profile's auth token is chosen from the AP's real
auth algorithm (`WPA2PSK` vs `WPA3SAE`).

**Audited live (2026-06-23)** against phone hotspots + a WPA3 AP: new-SSID WPA2 connect, wrong
password → `AuthFailed`, open (None), WPA3-SAE, plus `forget` and `ip_config`. All pass.

Deferred (not blockers): per-BSS `bssid`/`frequency_mhz` (needs `WlanGetNetworkBssList`); a
`wait_for_ip` helper (connect resolves at L2 association — DHCP IPv4 may lag a moment).

## Phase 2 — Profile management — [done] (2026-06-23)
- Windows: `saved_networks()` via `WlanGetProfileList` (+ `WlanGetProfile` to read each profile's
  `<authentication>` → `Security`; priority = OS preference order). `forget()` via
  `WlanDeleteProfile` (maps `ERROR_NOT_FOUND` → `NotFound`). `Network.saved` already populated in
  Phase 1.
- `connect` now picks the PSK profile's auth token (`WPA2PSK` vs `WPA3SAE`) from the AP's real
  auth algorithm (cached scan list), so WPA3-SAE-only networks are connectable.
- **Verified live:** `saved_networks`, `forget` (profile deleted + confirmed gone), and WPA3-vs-WPA2
  connect selection (audited 2026-06-23).

## Phase 3 — Connection events — [done] (2026-06-23)
- Windows: `subscribe()` returns an unbounded receiver of `WifiEvent::{ScanComplete, StateChanged,
  SignalChanged}`. A dedicated `wlanapi` handle registers ACM + MSM notifications; the OS callback
  maps connection-lifecycle / scan / signal codes onto events. The registration is a self-cleaning
  guard (closing the handle stops callbacks, then frees the sender), kept separate from the main
  handle so `scan`'s temporary registration never clobbers it.
- **Verified live:** scan → `ScanComplete` + `SignalChanged`; disconnect/reconnect → the full
  `Disconnecting → Disconnected → Connecting → Connected` sequence.
- Done (2026-06-23): `connect()` awaits `connection_complete` / security `connection_attempt_fail`
  and surfaces `AuthFailed` instead of returning on initiate (see Phase 1).

## IP details (`ip_config`) — [done] (2026-06-23)
- `ip_config()` on the `WifiBackend` trait (not a separate trait) returns `IpConfig { mac, ipv4,
  ipv6, gateway, dns }`. Windows reads it via the IP Helper API (`GetAdaptersAddresses`,
  `src/platform/windows/ipconfig.rs`), matching the wifi interface GUID. It is a **live query each
  call** (no caching — IP config changes via DHCP/roaming outside our `connect`); re-read after a
  `StateChanged(Connected)` event. Needs the `IpHelper` + `WinSock` windows features.
- **Verified live** (MAC, IPv4, IPv6 link-local, gateway, DNS all resolved).

## Phase 4 — Linux backend (NetworkManager) — [planned]
- `platform/linux.rs` over `zbus` (uncomment the gated dep). Mirror Phases 1–3 against
  `org.freedesktop.NetworkManager` (GetDevices / RequestScan / AddAndActivateConnection /
  Disconnect / Settings / signals).
- Keep the door open for a `wpa_supplicant` backend behind the same trait with runtime daemon
  detection — needed for minimal/headless qjay images that lack NetworkManager.

## Phase 5 — Advanced — [deferred]
- WPA2/WPA3-Enterprise (802.1X/EAP): firm up `Credentials::Enterprise`, per-backend EAP config.
- Hotspot / soft-AP mode (Windows Mobile Hotspot WinRT / NM AP mode). Largest platform divergence.

## Future
- **macOS** via CoreWLAN, replacing `DummyWifi`. Not Linux-like (Objective-C framework), so it's a
  separate backend, not a Linux variant.

## Non-goals
General network config (routing/VPN/Ethernet), a daemon, or a UI. See
[PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md#scope).
