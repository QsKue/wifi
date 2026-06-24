# Types

Source: `src/types.rs`. The backend-neutral vocabulary every layer speaks. Re-exported at the
crate root (`pub use types::*`).

## What belongs here
- Public value types: `Network`, `Interface`, `ConnectionState`, `ConnectionStatus`, `IpConfig`,
  `SavedNetwork`, `Security`, `Credentials`, `ConnectRequest`, `Connectivity`, `WifiEvent`.
- The error type `WifiError` + `Result<T>` alias.
- Small, pure conversions/normalization helpers that are platform-independent.

## What does not belong here
- Any OS-specific type or call (`windows::…`, `zbus::…`). Those stay in `platform/` and are mapped
  *to* these types there.
- The trait or the facade — those are [api.md](api.md).

## Conventions
- **Normalized units.** `signal` is always link quality `0–100` (`u8`), regardless of whether the
  OS reports quality or dBm — the backend converts. Frequencies are MHz.
- **Secrets don't print.** `Credentials` has a hand-written `Debug` that redacts the PSK/password.
  Keep it that way when adding variants.
- **Errors are backend-neutral.** Map OS failures onto a `WifiError` variant; put the raw
  platform detail in `OsApi(String)`. `Unimplemented(&'static str)` marks a recognized-but-unbuilt
  op (see [../ROADMAP.md](../ROADMAP.md)); `PlatformNotSupported` marks "no backend for this OS".
- `Security` is intentionally coarse (Open/Wep/Psk/Enterprise/Unknown) — it's an auth *category*,
  not the exact cipher suite.

## Gotchas / current state
- `Credentials::Enterprise` is a placeholder shape; its real fields land with Phase 5. Don't build
  against it yet.
- `Credentials::Saved` means "reconnect via the OS's existing profile" — backends must connect
  without staging/overwriting a profile, so it never alters stored credentials. It fails if no
  profile exists.
- `Network.bssid` is `None` for SSID-level entries and `Some` only when a specific AP is meant.
- Adding a field to a public struct is a breaking change for consumers (qjay/qshell) — note it.

## Update this file when
- A public type gains/loses a field or variant, the normalization rules change, or a new
  `WifiError` variant is added.
