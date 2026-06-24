//! Internet-reachability detection via WinRT `NetworkInformation` — the OS's NCSI verdict, the same
//! signal behind the tray "No internet" / captive-portal indicators. Nothing here probes the
//! network; Windows continuously determines reachability and this maps its level onto
//! `Connectivity`.
//!
//! WinRT statics are agile but require a live COM apartment on the process. `ensure_mta` keeps an
//! implicit process-wide MTA alive so these calls succeed from any tokio worker thread without
//! per-thread COM initialization.

use crate::types::{Connectivity, Result, WifiError};
use std::sync::OnceLock;
use windows::Foundation::EventRegistrationToken;
use windows::Networking::Connectivity::{
    NetworkConnectivityLevel, NetworkInformation, NetworkStatusChangedEventHandler,
};
use windows::Win32::System::Com::CoIncrementMTAUsage;

/// Keep an implicit process-wide MTA alive for the lifetime of the process. The cookie is
/// intentionally never released: the MTA must outlive every WinRT call the crate makes.
fn ensure_mta() {
    static MTA: OnceLock<()> = OnceLock::new();
    MTA.get_or_init(|| unsafe {
        let _ = CoIncrementMTAUsage();
    });
}

/// The OS's current internet-reachability level for the machine's active internet connection. When
/// no profile provides internet, the OS exposes no internet connection profile, which reads as
/// `Offline`.
pub fn current() -> Result<Connectivity> {
    ensure_mta();
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return Ok(Connectivity::Offline);
    };
    let level = profile
        .GetNetworkConnectivityLevel()
        .map_err(|e| WifiError::OsApi(format!("GetNetworkConnectivityLevel: {e}")))?;
    Ok(map_level(level))
}

/// `NetworkConnectivityLevel` is a WinRT enum (a struct of named constants, not Rust variants), so
/// it is matched by equality rather than a `match` arm.
fn map_level(level: NetworkConnectivityLevel) -> Connectivity {
    if level == NetworkConnectivityLevel::InternetAccess {
        Connectivity::Online
    } else if level == NetworkConnectivityLevel::ConstrainedInternetAccess {
        Connectivity::CaptivePortal
    } else if level == NetworkConnectivityLevel::LocalAccess {
        Connectivity::LocalOnly
    } else {
        Connectivity::Offline
    }
}

/// Register a handler invoked on every OS connectivity change with the freshly-queried level.
/// Returns the registration token, which must be passed to [`remove_status_changed`] to stop
/// callbacks and release the handler.
pub fn register_status_changed<F>(mut on_change: F) -> Result<EventRegistrationToken>
where
    F: FnMut(Connectivity) + Send + 'static,
{
    ensure_mta();
    let handler = NetworkStatusChangedEventHandler::new(move |_sender| {
        if let Ok(c) = current() {
            on_change(c);
        }
        Ok(())
    });
    NetworkInformation::NetworkStatusChanged(&handler)
        .map_err(|e| WifiError::OsApi(format!("NetworkStatusChanged: {e}")))
}

pub fn remove_status_changed(token: EventRegistrationToken) {
    let _ = NetworkInformation::RemoveNetworkStatusChanged(token);
}
