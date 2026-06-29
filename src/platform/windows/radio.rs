//! Wi-Fi radio power via WinRT `Windows.Devices.Radios` — the same switch the OS "Wi-Fi off" /
//! airplane-mode toggle drives. This powers the radio itself, distinct from scanning/associating:
//! with the radio off there are no emissions and the OS won't scan or connect.

use crate::types::{Result, WifiError};

use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};
use windows::Win32::System::Com::CoIncrementMTAUsage;

/// WinRT calls need a live MTA on the calling thread; hold the cookie for the process lifetime.
fn ensure_mta() {
    use std::sync::OnceLock;
    static MTA: OnceLock<()> = OnceLock::new();
    MTA.get_or_init(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

/// Map a WinRT failure to an [`WifiError::OsApi`] tagged with the call that produced it.
fn os(label: &'static str) -> impl FnOnce(windows::core::Error) -> WifiError {
    move |e| WifiError::OsApi(format!("{label}: {e}"))
}

/// True if any Wi-Fi radio is currently powered on.
pub fn is_enabled() -> Result<bool> {
    ensure_mta();
    let radios = Radio::GetRadiosAsync().map_err(os("GetRadiosAsync"))?.get().map_err(os("GetRadiosAsync"))?;
    for radio in radios {
        if radio.Kind().map_err(os("Radio.Kind"))? == RadioKind::WiFi
            && radio.State().map_err(os("Radio.State"))? == RadioState::On
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Power every Wi-Fi radio on or off — the OS "Wi-Fi off" / airplane switch. Requires the one-time
/// radio access grant (always allowed for a desktop app); a denial surfaces as an error.
pub fn set_enabled(on: bool) -> Result<()> {
    ensure_mta();
    let access = Radio::RequestAccessAsync()
        .map_err(os("RequestAccessAsync"))?
        .get()
        .map_err(os("RequestAccessAsync"))?;
    if access != RadioAccessStatus::Allowed {
        return Err(WifiError::OsApi(format!("radio access not allowed: {access:?}")));
    }

    let radios = Radio::GetRadiosAsync().map_err(os("GetRadiosAsync"))?.get().map_err(os("GetRadiosAsync"))?;
    let target = if on { RadioState::On } else { RadioState::Off };
    let mut found = false;
    for radio in radios {
        if radio.Kind().map_err(os("Radio.Kind"))? == RadioKind::WiFi {
            found = true;
            radio.SetStateAsync(target).map_err(os("SetStateAsync"))?.get().map_err(os("SetStateAsync"))?;
        }
    }
    if found {
        Ok(())
    } else {
        Err(WifiError::NotFound)
    }
}
