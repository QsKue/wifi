# Platform — Windows

Source: `src/platform/windows/` (`mod.rs` backend, `conv.rs` type mapping, `profile.rs` connect
XML, `ipconfig.rs` IP-Helper details). The `WindowsWifi` backend: the **Native WiFi API** (`wlanapi`, in
`windows::Win32::NetworkManagement::WiFi`) via the official `windows` crate. The active target;
core station ops are implemented and verified on real hardware.

## What belongs here
- `WlanOpenHandle`/`WlanCloseHandle` lifetime, `WlanEnumInterfaces`, `WlanScan`,
  `WlanGetAvailableNetworkList`, `WlanSetProfile`/`WlanConnect`, `WlanDisconnect`,
  `WlanQueryInterface`, profile list/delete, and `WlanRegisterNotification`.
- Mapping `WLAN_*` structs → `types.rs` (signal quality → `u8` 0–100; `DOT11_AUTH_ALGORITHM` /
  cipher → `Security`; interface GUID → `Interface.id` string).
- Building connect **profile XML** (the `wlanapi` connect path is profile-based).

## What does not belong here
- Anything backend-neutral (it's in [types.md](types.md)) or any cross-platform policy.
- Leaking a `windows::…` type out of this module — map before returning.

## Conventions
- Gated entirely by `#[cfg(windows)]` via `platform/mod.rs`; the `windows` dep is
  `[target.'cfg(windows)']` in `Cargo.toml` with only the needed features
  (`Win32_NetworkManagement_WiFi`, `Win32_Foundation`, `Win32_Security`). Add a feature when you
  call an API that needs it; keep the list minimal.
- Wrap each failed call as `WifiError::OsApi(format!("WlanXxx: {e:?}"))` — mirror
  `tools/bluetooth`'s `map_err` helper style.
- `wlanapi` is synchronous + callback-driven; keep the `async fn` surface by awaiting a
  oneshot/notify that the scan/connect notification fires, rather than blocking the executor.

## Gotchas / current state
- **All `WifiBackend` ops implemented** (`interfaces`/`scan`/`connect`/`disconnect`/`status`/
  `saved_networks`/`forget`/`subscribe`). The `windows` dep needs the `Ndis` feature, not just
  `WiFi`: `WlanConnect`/`WLAN_CONNECTION_PARAMETERS` are gated on it (the struct embeds
  `DOT11_BSSID_LIST`).
- `subscribe` opens a **dedicated** `wlanapi` handle (separate from the main one) and registers
  ACM + MSM notifications; the `EventReg` guard closes that handle on drop (stopping callbacks)
  before freeing the boxed sender. Keeping it off the main handle is deliberate — `scan` registers
  a temporary notification there, and one handle has only one callback slot.
- `saved_networks` reads security per profile via `WlanGetProfile` + a small `<authentication>`
  parse (`conv::security_from_profile_xml`); priority is the OS's profile-list order (highest first).
- `connect` reads the target's auth algorithm from the cached available-network list to choose the
  PSK profile flavour (`WPA2PSK` vs `WPA3SAE`) — no forced rescan; unknown falls back to `WPA2PSK`.
- `ip_config` uses a different API entirely — the IP Helper API (`GetAdaptersAddresses` in
  `ipconfig.rs`), matched to the wifi interface by GUID string. It's IP-layer (MAC/IP/gateway/DNS),
  queried live each call, not link-layer like the rest. Buffer is a `Vec<u64>` for alignment;
  `SOCKADDR` is rendered via `std::net::Ipv4Addr`/`Ipv6Addr`.
- `WlanConnect` needs a profile to exist first (`WlanSetProfile`) — connect is two calls, not one.
  `connect` then **awaits** the outcome: it registers for the terminal connection notification on a
  *dedicated* handle (registered before `WlanConnect` so a fast result isn't missed), returning `Ok`
  on `connection_complete` reason 0, or `AuthFailed` for a security-band reason
  (`0x40000..0x50000`, e.g. wrong key `0x48014`). A wrong key takes ~25s (Windows retries), so the
  await times out at 40s. `connect` resolves at L2 association — DHCP IPv4 may lag, so `ip_config`
  right after may show no IPv4 yet.
- The scan-complete callback runs on an OS thread; it signals an `mpsc` channel and the async side
  polls it with a `tokio::time` deadline (no executor-local state touched from the callback).
- Many calls allocate buffers freed with `WlanFreeMemory` — every allocating call is paired with
  its free.
- `scan` results currently carry no `bssid`/`frequency_mhz` (the available-network list is
  SSID-level); per-BSS detail needs `WlanGetNetworkBssList`. Entries are deduped by SSID.
- The client handle is wrapped to assert `Send`/`Sync` (Native WiFi handles are thread-safe), so
  `WiFi` can be shared across a multi-threaded runtime.

## Update this file when
- A new `wlanapi` call/sequence is added, the profile-XML shape changes, or the notification
  bridge changes.
