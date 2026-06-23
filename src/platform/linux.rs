//! Linux backend — NetworkManager over D-Bus (`org.freedesktop.NetworkManager`), via `zbus`. Wi-Fi
//! is driven through the NetworkManager D-Bus interface; `nmcli` is never invoked.
//!
//! Each operation maps to a NetworkManager call:
//! - construct: open a system-bus connection (`zbus::Connection::system()`).
//! - `interfaces`: `GetDevices`, filtered to `DeviceType == Wifi`.
//! - `scan`: `Device.Wireless.RequestScan`, then read `AccessPoints`.
//! - `connect`: `AddAndActivateConnection`, or activate an existing connection, with the SSID/PSK
//!   settings.
//! - `disconnect`: `Device.Disconnect`.
//! - `status`: read the device `State` / `ActiveConnection`.
//! - saved profiles: the `Settings` interface (`ListConnections` / `Delete`).
//! - events: subscribe to NetworkManager `PropertiesChanged` / `StateChanged` signals.
//!
//! NetworkManager is async and signal-driven, which fits the async surface directly. The impl is
//! kept factored so a second backend speaking `fi.w1.wpa_supplicant1` can sit behind the same
//! trait and be chosen by runtime daemon detection, for minimal images that lack NetworkManager.

use crate::interface::WifiBackend;
use crate::types::*;

pub struct LinuxWifi {
    // Holds the zbus system-bus connection and the cached NetworkManager proxies.
}

impl LinuxWifi {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

impl WifiBackend for LinuxWifi {
    async fn interfaces(&self) -> Result<Vec<Interface>> {
        Err(WifiError::Unimplemented("linux: interfaces (NM GetDevices)"))
    }

    async fn scan(&self) -> Result<Vec<Network>> {
        Err(WifiError::Unimplemented("linux: scan (NM RequestScan)"))
    }

    async fn connect(&self, _req: &ConnectRequest) -> Result<()> {
        Err(WifiError::Unimplemented("linux: connect (NM AddAndActivateConnection)"))
    }

    async fn disconnect(&self) -> Result<()> {
        Err(WifiError::Unimplemented("linux: disconnect (NM Device.Disconnect)"))
    }

    async fn status(&self) -> Result<ConnectionStatus> {
        Err(WifiError::Unimplemented("linux: status (NM device State)"))
    }
}
