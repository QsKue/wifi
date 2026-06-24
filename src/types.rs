//! Backend-neutral value types. These are the public vocabulary of the crate; every
//! platform backend maps the OS's native structures onto these and never leaks its own.

use std::fmt;

/// The security/authentication scheme of a network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    /// No authentication.
    Open,
    /// Legacy WEP (treat as good as open; supported for completeness).
    Wep,
    /// WPA/WPA2/WPA3 with a pre-shared key (the common "home Wi-Fi password" case).
    Psk,
    /// WPA2/WPA3-Enterprise (802.1X / EAP). Credentials carry identity material.
    Enterprise,
    /// Present but unrecognized.
    Unknown,
}

/// A network as seen in a scan, or a remembered/saved one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub ssid: String,
    /// BSSID (AP MAC, "XX:XX:XX:XX:XX:XX") when a specific AP is known; `None` for an
    /// SSID-level/aggregated entry.
    pub bssid: Option<String>,
    pub security: Security,
    /// Link quality 0–100 (normalized across platforms). `None` for saved-but-not-seen entries.
    pub signal: Option<u8>,
    /// Center frequency in MHz when known (e.g. 2412, 5180).
    pub frequency_mhz: Option<u32>,
    /// True if the OS already has a saved profile for this SSID.
    pub saved: bool,
}

/// A wireless interface/adapter on the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Opaque per-platform id: an interface GUID on Windows, the device path/ifname on Linux.
    pub id: String,
    /// Human-readable description ("Intel Wi-Fi 6 AX201").
    pub description: String,
    pub state: ConnectionState,
}

/// Where an interface (or the facade's active interface) is in the connection lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

/// Credentials supplied when joining a network.
#[derive(Clone)]
pub enum Credentials {
    /// Open network — no secret.
    None,
    /// WPA/WPA2/WPA3 pre-shared key (passphrase).
    Psk(String),
    /// 802.1X / EAP material. The field set is provisional and will expand to cover the EAP
    /// methods each backend supports.
    Enterprise { identity: String, password: String },
    /// Join using the OS's existing stored profile for this SSID, supplying no secret. Fails if no
    /// profile exists. This is the reconnect-to-a-known-network path; it never alters the stored
    /// credentials.
    Saved,
}

// Keep secrets out of logs.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Credentials::None => write!(f, "Credentials::None"),
            Credentials::Psk(_) => write!(f, "Credentials::Psk(***)"),
            Credentials::Enterprise { identity, .. } => {
                write!(f, "Credentials::Enterprise {{ identity: {identity:?}, password: *** }}")
            }
            Credentials::Saved => write!(f, "Credentials::Saved"),
        }
    }
}

/// A request to join a network.
#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub ssid: String,
    pub credentials: Credentials,
    /// Join even if the SSID is not currently broadcast.
    pub hidden: bool,
    /// Pin to a specific AP by BSSID; `None` lets the OS pick the best.
    pub bssid: Option<String>,
}

/// A snapshot of what the active interface is currently doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionStatus {
    pub state: ConnectionState,
    /// The SSID we're on/joining, when applicable.
    pub ssid: Option<String>,
    /// Current link quality 0–100, when connected.
    pub signal: Option<u8>,
}

/// IP-layer configuration of the active interface — the adapter MAC plus the addresses assigned to
/// the current connection. Distinct from [`ConnectionStatus`] (which is link-layer) because this
/// comes from the OS networking stack, not the Wi-Fi radio. Fields are whatever the OS currently
/// reports: empty/`None` when not connected or not yet assigned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IpConfig {
    /// The adapter's own MAC address, "AA:BB:CC:DD:EE:FF".
    pub mac: Option<String>,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    /// Default gateway (first one reported), when assigned.
    pub gateway: Option<String>,
    pub dns: Vec<String>,
}

/// A network the OS remembers (has a stored profile for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedNetwork {
    pub ssid: String,
    pub security: Security,
    /// Auto-connect preference (higher = preferred) when the platform exposes it.
    pub priority: Option<i32>,
}

/// Internet reachability of the active connection, as the OS itself determines it (Windows NCSI /
/// NetworkManager connectivity check). Distinct from [`ConnectionState`]: a link can be `Connected`
/// yet `Offline` (associated to an AP with no working internet, e.g. a hotspot with no upstream) or
/// `CaptivePortal` (a login page is intercepting traffic). The OS does the probing; nothing here
/// reaches out to the network itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// No internet path (may still be associated to an AP with local-only access).
    Offline,
    /// Connected, but the OS sees only the local network — no internet.
    LocalOnly,
    /// Internet present but a captive portal (login page) is intercepting traffic.
    CaptivePortal,
    /// Full internet reachability.
    Online,
}

/// Live notifications from the active interface, delivered over the channel returned by
/// [`crate::WifiBackend::subscribe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiEvent {
    /// A fresh scan completed; call `scan()` to read the results.
    ScanComplete,
    /// The connection lifecycle advanced.
    StateChanged(ConnectionState),
    /// Link quality changed (0–100).
    SignalChanged(u8),
    /// Internet reachability changed (online / offline / local-only / captive portal).
    Connectivity(Connectivity),
}

/// Errors surfaced by the crate. Backend-neutral; platform detail rides in `OsApi`.
#[derive(Debug)]
pub enum WifiError {
    /// This OS has no native backend.
    PlatformNotSupported(&'static str),
    /// A recognized operation the active backend does not yet provide.
    Unimplemented(&'static str),
    /// No matching interface / network / profile.
    NotFound,
    /// Caller passed something invalid (bad SSID, malformed BSSID, …).
    InvalidArgument(&'static str),
    /// The network rejected our credentials.
    AuthFailed,
    /// An OS API call failed; string carries the platform-specific detail.
    OsApi(String),
    Other(String),
}

impl fmt::Display for WifiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WifiError::PlatformNotSupported(m) => write!(f, "Platform not supported: {m}"),
            WifiError::Unimplemented(m) => write!(f, "Not implemented: {m}"),
            WifiError::NotFound => write!(f, "Not found"),
            WifiError::InvalidArgument(m) => write!(f, "Invalid argument: {m}"),
            WifiError::AuthFailed => write!(f, "Authentication failed"),
            WifiError::OsApi(m) => write!(f, "OS API error: {m}"),
            WifiError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for WifiError {}

pub type Result<T> = std::result::Result<T, WifiError>;
