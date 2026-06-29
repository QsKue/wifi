//! `WiFi` — the public facade. Owns the compile-time-selected platform [`Backend`] and forwards
//! every call to it. This is the type callers construct; they never name a platform struct.

use crate::interface::WifiBackend;
use crate::platform::Backend;
use crate::types::*;

pub struct WiFi {
    backend: Backend,
}

impl WiFi {
    /// Open a handle to the OS Wi-Fi subsystem. Errors with `PlatformNotSupported` on a target
    /// that has no native backend.
    pub fn new() -> Result<Self> {
        Ok(Self { backend: Backend::new()? })
    }

    pub async fn interfaces(&self) -> Result<Vec<Interface>> {
        self.backend.interfaces().await
    }

    pub async fn scan(&self) -> Result<Vec<Network>> {
        self.backend.scan().await
    }

    /// The OS's cached list of visible networks — returns immediately, no fresh scan forced. See
    /// [`WifiBackend::available_networks`].
    pub async fn available_networks(&self) -> Result<Vec<Network>> {
        self.backend.available_networks().await
    }

    pub async fn connect(&self, req: &ConnectRequest) -> Result<()> {
        self.backend.connect(req).await
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.backend.disconnect().await
    }

    pub async fn status(&self) -> Result<ConnectionStatus> {
        self.backend.status().await
    }

    /// IP-layer configuration of the active interface (MAC, addresses, gateway, DNS).
    pub async fn ip_config(&self) -> Result<IpConfig> {
        self.backend.ip_config().await
    }

    pub async fn saved_networks(&self) -> Result<Vec<SavedNetwork>> {
        self.backend.saved_networks().await
    }

    pub async fn forget(&self, ssid: &str) -> Result<()> {
        self.backend.forget(ssid).await
    }

    /// Subscribe to live connection events for the active interface.
    pub fn subscribe(&self) -> Result<tokio::sync::mpsc::UnboundedReceiver<WifiEvent>> {
        self.backend.subscribe()
    }

    /// The OS's current internet-reachability verdict (online / offline / local-only / captive
    /// portal). Live changes also arrive as [`WifiEvent::Connectivity`] on the `subscribe` stream.
    pub async fn connectivity(&self) -> Result<Connectivity> {
        self.backend.connectivity().await
    }
}
