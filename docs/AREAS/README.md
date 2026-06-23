# Area docs

One doc per real module. Read the one you're touching; each lists what belongs there, the
conventions, and the current gotchas.

- [types.md](types.md) — `src/types.rs`: the backend-neutral vocabulary (values + errors).
- [api.md](api.md) — `src/wifi.rs` + `src/interface.rs`: the `WiFi` facade and the `WifiBackend`
  trait (the public API and the per-platform contract).
- [platform-windows.md](platform-windows.md) — `src/platform/windows/`: the Native WiFi
  (`wlanapi`) backend.
- [platform-linux.md](platform-linux.md) — `src/platform/linux.rs`: the NetworkManager (D-Bus)
  backend.

`src/platform/dummy.rs` is trivial (returns `PlatformNotSupported`) and has no area doc; it's
described in [../ARCHITECTURE.md](../ARCHITECTURE.md).
