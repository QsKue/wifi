//! Manual audit runner for the new-SSID connect paths. Scans, finds the target SSID, connects with
//! the given credentials, prints status + ip_config, forgets the profile, then reconnects to the
//! restore network (saved) so internet returns.
//!
//! Configure via env (no hardcoded networks/passwords):
//!   WIFI_TEST_SSID=<ssid>  WIFI_TEST_PSK=<password>  WIFI_RESTORE_SSID=<your-saved-ssid>
//! Optional: WIFI_TEST_OPEN=1 (connect with no password), WIFI_TEST_KEEP=1 (skip forget/restore).
//! `cargo run --example audit`

use std::time::Duration;
use wifi::{ConnectRequest, Credentials, WiFi};

fn env_req(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::main]
async fn main() -> wifi::Result<()> {
    let target = env_req("WIFI_TEST_SSID");
    let open = std::env::var("WIFI_TEST_OPEN").is_ok();
    let psk = if open { String::new() } else { env_req("WIFI_TEST_PSK") };
    let restore = env_req("WIFI_RESTORE_SSID");
    // Skip forget+restore — for testing the very network we want to stay connected to.
    let keep = std::env::var("WIFI_TEST_KEEP").is_ok();

    let wifi = WiFi::new()?;

    // 1. scan and locate the target.
    println!("scanning for {target:?}...");
    let networks = wifi.scan().await?;
    match networks.iter().find(|n| n.ssid == target) {
        Some(n) => println!("  found: {n:?}"),
        None => {
            println!("  NOT in scan results — visible networks:");
            for n in &networks {
                println!("    {} ({:?}, signal {:?})", n.ssid, n.security, n.signal);
            }
            println!("  aborting; nothing to connect to.");
            return Ok(());
        }
    }

    // 2. connect (awaits the real outcome now).
    let credentials = if open { Credentials::None } else { Credentials::Psk(psk) };
    println!("connecting to {target:?} ({})...", if open { "open" } else { "psk" });
    match wifi
        .connect(&ConnectRequest {
            ssid: target.clone(),
            credentials,
            hidden: false,
            bssid: None,
        })
        .await
    {
        Ok(()) => {
            println!("  connect OK");
            println!("  status:    {:?}", wifi.status().await?);
            println!("  ip_config: {:?}", wifi.ip_config().await?);
        }
        Err(e) => println!("  connect FAILED: {e}"),
    }

    if keep {
        println!("keep mode: staying connected to {target:?}, skipping forget/restore.");
        return Ok(());
    }

    // 3. forget the test profile and confirm it's gone.
    println!("forgetting {target:?}...");
    match wifi.forget(&target).await {
        Ok(()) => println!("  forget OK"),
        Err(e) => println!("  forget result: {e}"),
    }
    let still = wifi.saved_networks().await?.iter().any(|n| n.ssid == target);
    println!("  still saved? {still}");

    // 4. restore the working connection so internet returns.
    println!("restoring {restore:?} (saved)...");
    wifi.connect(&ConnectRequest {
        ssid: restore.clone(),
        credentials: Credentials::Saved,
        hidden: false,
        bssid: None,
    })
    .await
    .ok();
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("  final status: {:?}", wifi.status().await?);

    Ok(())
}
