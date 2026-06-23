# Platform — Linux

Source: `src/platform/linux.rs`. The `LinuxWifi` backend: **NetworkManager** over D-Bus
(`org.freedesktop.NetworkManager`) via `zbus`. Planned (Phase 4). No `nmcli` shelling.

## What belongs here
- A `zbus` system-bus connection + NM proxies; `GetDevices` (filter `DeviceType == Wifi`),
  `Device.Wireless.RequestScan` + `AccessPoints`, `AddAndActivateConnection`/activate,
  `Device.Disconnect`, the `Settings` interface (list/delete profiles), and NM signal
  subscriptions for events.
- Mapping NM access points / device state → `types.rs` (`802-11-wireless` settings → `Security`;
  NM `Strength` 0–100 → `signal`; device path → `Interface.id`).

## What does not belong here
- Backend-neutral logic ([types.md](types.md)) or Windows specifics.
- A `zbus`/NM type in the public API — map before returning.

## Conventions
- Will be gated by `#[cfg(target_os = "linux")]`; the `zbus` dep is `[target.'cfg(target_os =
  "linux")']` in `Cargo.toml`, **currently commented** — uncomment when implementation starts so
  Linux builds stay lean until then.
- NM is the starting point because it's present on most desktops and the dev images. Keep the impl
  factored so a second backend (`wpa_supplicant`, `fi.w1.wpa_supplicant1`) can sit behind the same
  `WifiBackend` trait with **runtime daemon detection** — minimal/headless qjay images may not have
  NM. See [../ROADMAP.md](../ROADMAP.md) and ADR [0001](../DECISIONS/0001-initial-architecture.md).

## Gotchas / current state
- **Skeleton only.** Methods return `WifiError::Unimplemented("linux: …")`; source comments name
  the NM call for each.
- NM is async/signal-based — fits the `async fn` surface directly; events come from
  `PropertiesChanged`/`StateChanged` rather than polling.
- Connecting may require an existing connection profile or creating one inline
  (`AddAndActivateConnection`); decide per call whether to reuse or create.

## Update this file when
- The Linux backend lands or changes, a `wpa_supplicant` backend is added, or the daemon-detection
  strategy changes.
