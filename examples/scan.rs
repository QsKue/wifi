//! Minimal smoke test: open the OS Wi-Fi subsystem and print interfaces + a scan.
//! `cargo run --example scan`

#[tokio::main]
async fn main() -> wifi::Result<()> {
    let wifi = wifi::WiFi::new()?;

    println!("interfaces:     {:?}", wifi.interfaces().await);
    println!("scan:           {:?}", wifi.scan().await);
    println!("status:         {:?}", wifi.status().await);
    println!("ip_config:      {:#?}", wifi.ip_config().await);
    println!("saved_networks: {:?}", wifi.saved_networks().await);

    Ok(())
}
