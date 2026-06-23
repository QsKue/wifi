# AI Context

The first thing to read before working on `wifi`. Compressed context + the required workflow.

## In one paragraph
`wifi` is a small cross-platform Rust crate for controlling the machine's Wi-Fi adapter — scan,
connect, disconnect, status, saved-network management — through each OS's **native programming
interface**, never by parsing CLI tool output. It exists to replace `tools/wifi-rs`, which shells
out to `netsh`/`nmcli`. The API is **async** (tokio). Callers construct one `WiFi` facade; a single
platform backend is bound at compile time behind the `WifiBackend` trait. **Windows** (Native WiFi
API / `wlanapi`) is the live target; **Linux** (NetworkManager over D-Bus) is planned;
**macOS/other** get a `PlatformNotSupported` stub. Consumers: **qjay** (player appliance) and
**qshell**.

## Shape to hold in your head
- **One facade, one backend, chosen at compile time.** `WiFi` (`wifi.rs`) wraps `platform::Backend`,
  which `platform/mod.rs` aliases via `#[cfg]` to exactly one of `WindowsWifi` / `LinuxWifi` /
  `DummyWifi`. No `dyn`, no runtime backend selection. ([ARCHITECTURE.md](ARCHITECTURE.md))
- **The trait is the contract; types are the vocabulary.** Every backend implements
  `WifiBackend` (`interface.rs`) and speaks only in `types.rs` values — no `windows`/`zbus` type
  ever crosses out of `platform/`. ([AREAS/types.md](AREAS/types.md))
- **Phased and honest.** Unimplemented ops return `WifiError::Unimplemented(_)`. The skeleton
  compiles and runs on every target; only Windows is being filled in first.
  ([ROADMAP.md](ROADMAP.md))

## Workflow for a change
1. **Read** this file, [ARCHITECTURE.md](ARCHITECTURE.md), and the [AREA doc](AREAS/README.md) for
   the module you're touching (a platform backend, the trait/facade, or the types).
2. **Respect the guardrails** in [../AGENTS.md](../AGENTS.md): OS-native only (no CLI shelling);
   backends map to `types.rs` before returning; standalone-buildable; default build stays green.
3. **Make the change.** Add a capability to the `WifiBackend` trait first (with an `Unimplemented`
   default if not all backends will have it at once), forward it through the `WiFi` facade, then
   implement per platform. Map OS structures onto `types.rs` inside the backend.
4. **Verify**: `cargo check -p wifi` (and `--examples`) on your platform. Run `cargo run --example
   scan` against a real adapter when implementing a backend.
5. **Update docs** only if you changed lasting structure/conventions/decisions — the matching area
   doc or a new ADR. Move the relevant [ROADMAP.md](ROADMAP.md) item from planned to built.
   Otherwise leave docs alone; the commit message is the task log.

## What NOT to do
- Don't add a dependency on a CLI tool or `std::process::Command` for Wi-Fi control.
- Don't return placeholder/fake data for an unbuilt op — return `Unimplemented`.
- Don't leak a `windows`/`zbus` type through the public API.
- Don't add a per-task history doc; git is the log.
