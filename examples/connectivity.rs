//! Print current internet reachability, then stream live events. Toggle Wi-Fi / internet during
//! the listen window to see push updates. Listen seconds via WIFI_LISTEN_SECS (default 30).
//! `cargo run --example connectivity`

use std::time::Duration;
use wifi::WiFi;

#[tokio::main]
async fn main() -> wifi::Result<()> {
    let wifi = WiFi::new()?;
    println!("connectivity now: {:?}", wifi.connectivity().await?);

    let secs: u64 = std::env::var("WIFI_LISTEN_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let mut rx = wifi.subscribe()?;
    println!("listening {secs}s for events (toggle internet/Wi-Fi to see changes)...");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Some(ev) => println!("  event: {ev:?}"),
                None => break,
            },
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
    println!("done.");
    Ok(())
}
