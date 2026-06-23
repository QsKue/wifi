//! Connection profile XML. `WlanConnect` is profile-based: a network must have a stored profile
//! before it can be joined, so a connect stages one with `WlanSetProfile`. This builds the minimal
//! profile that the Native WiFi schema requires for the supported security types.

use crate::types::Credentials;

/// Build a WLAN profile XML document for `ssid` with the given credentials. The profile name is
/// the SSID, so a later connect or delete can address it by SSID. Caller-supplied text is
/// XML-escaped; the PSK is embedded unprotected (the OS stores it encrypted once accepted).
///
/// `wpa3` selects the PSK authentication token: a WPA3-SAE-only AP rejects a `WPA2PSK` profile, so
/// `true` emits `WPA3SAE`. It is ignored for non-PSK credentials.
pub(crate) fn profile_xml(ssid: &str, credentials: &Credentials, wpa3: bool) -> String {
    let name = escape(ssid);
    let security = match credentials {
        Credentials::None => OPEN_SECURITY.to_string(),
        Credentials::Psk(key) => psk_security(&escape(key), wpa3),
        // Enterprise (rejected earlier) and Saved (connects without staging a profile) never reach
        // this builder; the arm keeps the match exhaustive.
        Credentials::Enterprise { .. } | Credentials::Saved => OPEN_SECURITY.to_string(),
    };

    format!(
        r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{name}</name>
  <SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>auto</connectionMode>
  <MSM><security>{security}</security></MSM>
</WLANProfile>"#
    )
}

const OPEN_SECURITY: &str = "<authEncryption>\
<authentication>open</authentication>\
<encryption>none</encryption>\
<useOneX>false</useOneX>\
</authEncryption>";

/// WPA2-PSK / AES is the interoperable default and the broadest match for "the network password";
/// `wpa3` switches the authentication to `WPA3SAE` for access points that only accept SAE.
fn psk_security(escaped_key: &str, wpa3: bool) -> String {
    let authentication = if wpa3 { "WPA3SAE" } else { "WPA2PSK" };
    format!(
        "<authEncryption>\
<authentication>{authentication}</authentication>\
<encryption>AES</encryption>\
<useOneX>false</useOneX>\
</authEncryption>\
<sharedKey>\
<keyType>passPhrase</keyType>\
<protected>false</protected>\
<keyMaterial>{escaped_key}</keyMaterial>\
</sharedKey>"
    )
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
