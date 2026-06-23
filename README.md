# wifi

Cross-platform Wi-Fi station control for Rust, through **OS-native APIs** — not by scraping
`netsh` / `nmcli` output. Scan, connect, disconnect, query status, manage saved networks.

- **Windows** — Native WiFi API (`wlanapi`) via the `windows` crate. *(active target)*
- **Linux** — NetworkManager over D-Bus via `zbus`. *(planned)*
- **macOS / other** — a no-op backend reporting `PlatformNotSupported`. *(future: CoreWLAN)*

```rust
#[tokio::main]
async fn main() -> wifi::Result<()> {
    let wifi = wifi::WiFi::new()?;
    for net in wifi.scan().await? {
        println!("{} ({:?}) {:?}", net.ssid, net.security, net.signal);
    }
    wifi.connect(&wifi::ConnectRequest {
        ssid: "my-network".into(),
        credentials: wifi::Credentials::Psk("password".into()),
        hidden: false,
        bssid: None,
    }).await?;
    Ok(())
}
```

Async (tokio). Construct a `WiFi`; the platform backend is selected at compile time.

Status and what's implemented vs. planned: [docs/ROADMAP.md](docs/ROADMAP.md).
Architecture and contributor guide: [AGENTS.md](AGENTS.md) and [docs/](docs/README.md).
