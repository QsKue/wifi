//! Fallback backend for targets with no native implementation (such as macOS). It compiles
//! everywhere and reports `PlatformNotSupported` for every operation, so a caller on an
//! unsupported OS degrades gracefully at runtime instead of failing to build. A real backend for
//! such a target (e.g. CoreWLAN on macOS) replaces this alias rather than extending it.

use crate::interface::WifiBackend;
use crate::types::*;

const MSG: &str = "no native Wi-Fi backend for this OS";

pub struct DummyWifi;

impl DummyWifi {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl WifiBackend for DummyWifi {
    async fn interfaces(&self) -> Result<Vec<Interface>> {
        Err(WifiError::PlatformNotSupported(MSG))
    }
    async fn scan(&self) -> Result<Vec<Network>> {
        Err(WifiError::PlatformNotSupported(MSG))
    }
    async fn connect(&self, _req: &ConnectRequest) -> Result<()> {
        Err(WifiError::PlatformNotSupported(MSG))
    }
    async fn disconnect(&self) -> Result<()> {
        Err(WifiError::PlatformNotSupported(MSG))
    }
    async fn status(&self) -> Result<ConnectionStatus> {
        Err(WifiError::PlatformNotSupported(MSG))
    }
}
