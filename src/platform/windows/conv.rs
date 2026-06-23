//! Conversions between `wlanapi` structures and the crate's neutral [`crate::types`]. Every value
//! a `wlanapi` call produces is translated here before it leaves the Windows backend, so no
//! `windows` type reaches the public API.

use crate::types::*;
use windows::Win32::NetworkManagement::WiFi::{DOT11_SSID, WLAN_INTERFACE_STATE};
use windows::core::GUID;

/// Map a `WIN32_ERROR` return code (0 == success) to a `Result`. The label names the call so the
/// raw code is actionable.
pub(crate) fn check(label: &str, ret: u32) -> Result<()> {
    if ret == 0 {
        Ok(())
    } else {
        Err(WifiError::OsApi(format!("{label}: WIN32_ERROR {ret}")))
    }
}

/// Decode a fixed-size, NUL-terminated UTF-16 field (interface descriptions, profile names) into a
/// `String`, stopping at the first NUL.
pub(crate) fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Decode a `DOT11_SSID` (length-prefixed bytes, not NUL-terminated) into a `String`.
pub(crate) fn ssid_to_string(ssid: &DOT11_SSID) -> String {
    let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
    String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned()
}

/// Format a `GUID` as the canonical `{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}` interface id.
pub(crate) fn guid_to_string(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7],
    )
}

/// Translate an interface's `WLAN_INTERFACE_STATE` into the neutral lifecycle. The transient
/// states (`associating`/`discovering`/`authenticating`) collapse to `Connecting`; everything
/// that is neither connected nor mid-transition reads as `Disconnected`.
pub(crate) fn state_from(state: WLAN_INTERFACE_STATE) -> ConnectionState {
    // Win32 WLAN_INTERFACE_STATE values are stable: 1 connected, 3 disconnecting, 4 disconnected,
    // 5 associating, 6 discovering, 7 authenticating.
    match state.0 {
        1 => ConnectionState::Connected,
        3 => ConnectionState::Disconnecting,
        5 | 6 | 7 => ConnectionState::Connecting,
        _ => ConnectionState::Disconnected,
    }
}

/// Derive the neutral [`Security`] from an available network's "security enabled" flag and its
/// default auth algorithm. An unsecured network is `Open` regardless of the algorithm; otherwise
/// the `DOT11_AUTH_ALGORITHM` value selects PSK vs. Enterprise vs. legacy WEP.
pub(crate) fn security_from(security_enabled: bool, auth_algo: i32) -> Security {
    if !security_enabled {
        return Security::Open;
    }
    // Stable DOT11_AUTH_ALGORITHM values: 1 open, 2 shared-key, 3 WPA, 4 WPA-PSK, 6 RSNA,
    // 7 RSNA-PSK, 8 WPA3, 9 WPA3-SAE, 11 WPA3-ENT.
    match auth_algo {
        4 | 7 | 9 => Security::Psk,
        3 | 6 | 8 | 11 => Security::Enterprise,
        1 | 2 => Security::Wep,
        _ => Security::Unknown,
    }
}

/// Classify a stored profile from the `<authentication>` element of its `WlanGetProfile` XML.
/// The Native WiFi schema uses fixed tokens (`open`, `WEP`, `WPA2PSK`, `WPA3SAE`, `WPA2`, …).
pub(crate) fn security_from_profile_xml(xml: &str) -> Security {
    let Some(auth) = inner_text(xml, "authentication") else {
        return Security::Unknown;
    };
    match auth.to_ascii_uppercase().as_str() {
        "OPEN" => Security::Open,
        "WEP" | "SHARED" => Security::Wep,
        a if a.ends_with("PSK") || a == "WPA3SAE" => Security::Psk,
        // WPA / WPA2 / WPA3 / WPA3ENT without the PSK suffix are the 802.1X (Enterprise) forms.
        a if a.starts_with("WPA") => Security::Enterprise,
        _ => Security::Unknown,
    }
}

/// Extract the text inside the first `<tag>…</tag>` pair, or `None` if absent. Tolerant of the
/// minimal, well-formed profile XML the OS returns (no attributes on these elements).
fn inner_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}
