# API — facade & backend trait

Source: `src/wifi.rs` (the `WiFi` facade) and `src/interface.rs` (the `WifiBackend` trait). The
public surface callers use, and the contract platform backends fulfill.

## What belongs here
- **`WiFi`** (`wifi.rs`): the type callers construct. Holds one `platform::Backend`, exposes the
  async ops, and forwards each to the backend. Thin by design — no logic beyond delegation.
- **`WifiBackend`** (`interface.rs`): the trait every backend implements. Required core ops
  (`interfaces`, `scan`, `connect`, `disconnect`, `status`) + default-`Unimplemented` later-phase
  ops (`saved_networks`, `forget`, `subscribe`).

## What does not belong here
- OS calls — those live in `platform/` ([platform-windows.md](platform-windows.md),
  [platform-linux.md](platform-linux.md)).
- Value/error definitions — those are [types.md](types.md).

## Conventions
- **Add a capability trait-first.** New op → add to `WifiBackend` (with a default `Unimplemented`
  body if not every backend will implement it immediately) → forward through `WiFi` → implement per
  platform. Keep the facade method signature identical to the trait method.
- **No `dyn`.** The facade binds the concrete `Backend` chosen by `platform/mod.rs` `#[cfg]`. This
  is why native `async fn` in the trait works; the `#[allow(async_fn_in_trait)]` is intentional
  (single concrete impl per build, so the "auto-trait leakage" caveat doesn't apply).
- **Construction can fail.** `WiFi::new()` / `Backend::new()` return `Result` — opening the OS
  handle (Windows `WlanOpenHandle`, Linux system bus) is fallible.

## Gotchas / current state
- All ops currently delegate to a backend that returns `Unimplemented`/`PlatformNotSupported`.
  The facade itself is complete; filling backends is the work ([../ROADMAP.md](../ROADMAP.md)).
- The facade operates on the adapter's **active/primary interface** implicitly (except
  `interfaces()`). Multi-interface targeting, if needed, is a future signature change — flag it as
  breaking.

## Update this file when
- A method is added/removed on the trait or facade, the default-impl set changes, or the
  construction/dispatch model changes.
