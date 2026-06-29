//! The contract every platform backend implements. The [`crate::WiFi`] facade binds exactly one
//! concrete impl at compile time (`#[cfg]`) and delegates to it. There is no `dyn` dispatch, which
//! is what makes native `async fn` in the trait sound here: a single concrete implementor per
//! build means the usual auto-trait-leakage caveat does not apply.
//!
//! Core station ops are required. The remaining ops carry default `Unimplemented` bodies so a
//! backend can implement the core set without blocking on profile or event support.

use crate::types::*;

#[allow(async_fn_in_trait)]
pub trait WifiBackend {
    // Core station operations — every backend must provide these.

    /// Enumerate the machine's wireless interfaces.
    async fn interfaces(&self) -> Result<Vec<Interface>>;

    /// Trigger a scan and return the visible networks on the active interface. Blocks until the
    /// scan completes (or a short timeout), so prefer [`available_networks`] for the first paint and
    /// use this to refresh.
    async fn scan(&self) -> Result<Vec<Network>>;

    /// The OS's *cached* list of currently-visible networks — returns immediately, without forcing a
    /// fresh scan (what the system Wi-Fi flyout shows the instant it opens). Pair with [`scan`], or a
    /// [`WifiEvent::ScanComplete`] from [`subscribe`], to refresh it.
    ///
    /// [`subscribe`]: WifiBackend::subscribe
    async fn available_networks(&self) -> Result<Vec<Network>> {
        Err(WifiError::Unimplemented("available_networks"))
    }

    /// Ask the OS to start a scan and return immediately, *without* waiting for it to finish —
    /// results arrive later as a [`WifiEvent::ScanComplete`] on the [`subscribe`] stream, after
    /// which [`available_networks`] reflects them. Use this to keep the cache warm cheaply; use
    /// [`scan`] when you need to block until fresh results are in hand.
    ///
    /// [`subscribe`]: WifiBackend::subscribe
    async fn request_scan(&self) -> Result<()> {
        Err(WifiError::Unimplemented("request_scan"))
    }

    /// Join a network. Creates/updates a profile as the OS requires.
    async fn connect(&self, req: &ConnectRequest) -> Result<()>;

    /// Drop the current association.
    async fn disconnect(&self) -> Result<()>;

    /// What the active interface is currently doing.
    async fn status(&self) -> Result<ConnectionStatus>;

    /// IP-layer configuration of the active interface (MAC, addresses, gateway, DNS). A live query
    /// — re-read it after a `StateChanged(Connected)` event rather than caching.
    async fn ip_config(&self) -> Result<IpConfig> {
        Err(WifiError::Unimplemented("ip_config"))
    }

    // Profile management — operates on the OS's stored network profiles.

    /// Networks the OS has stored profiles for.
    async fn saved_networks(&self) -> Result<Vec<SavedNetwork>> {
        Err(WifiError::Unimplemented("saved_networks"))
    }

    /// Delete a stored profile.
    async fn forget(&self, _ssid: &str) -> Result<()> {
        Err(WifiError::Unimplemented("forget"))
    }

    // Live connection events.

    /// Subscribe to live [`WifiEvent`]s for the active interface. The returned receiver stays open
    /// until dropped; the backend owns the underlying OS notification registration for its lifetime.
    /// Unbounded so the OS callback thread can enqueue without blocking.
    fn subscribe(&self) -> Result<tokio::sync::mpsc::UnboundedReceiver<WifiEvent>> {
        Err(WifiError::Unimplemented("subscribe"))
    }

    // Internet reachability.

    /// The OS's current internet-reachability verdict for the active connection. Cheap to call; the
    /// OS keeps this state continuously, so this only reads it. Live changes also arrive as
    /// [`WifiEvent::Connectivity`] on the `subscribe` stream.
    async fn connectivity(&self) -> Result<Connectivity> {
        Err(WifiError::Unimplemented("connectivity"))
    }
}
