//! `wifi` — cross-platform Wi-Fi station control via OS-native APIs, never CLI scraping.
//!
//! - Windows: the Native WiFi API (`wlanapi`) through the `windows` crate.
//! - Linux: NetworkManager over D-Bus.
//! - Other targets: a no-op backend that reports `PlatformNotSupported`.
//!
//! Construct a [`WiFi`] and call its async methods. Exactly one platform backend is bound at
//! compile time, so the public surface is identical on every target.

mod interface;
mod platform;
mod types;
mod wifi;

pub use interface::WifiBackend;
pub use types::*;
pub use wifi::WiFi;

/// The OS's current internet-reachability verdict — **synchronous**, no [`WiFi`] instance or async
/// runtime needed. Reads the cached NCSI level on Windows (the signal behind the tray "No internet"
/// indicator); other targets report [`Connectivity::Offline`] (no sync probe available). Intended
/// for cheap, frequent gating, e.g. "don't start an update with no internet".
pub fn connectivity() -> Result<Connectivity> {
    #[cfg(windows)]
    {
        platform::connectivity_current()
    }
    #[cfg(not(windows))]
    {
        Ok(Connectivity::Offline)
    }
}
