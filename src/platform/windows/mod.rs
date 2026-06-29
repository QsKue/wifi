//! Windows backend — the Native WiFi API (`wlanapi.dll`) via the official `windows` crate. Wi-Fi
//! is driven entirely through `Wlan*` calls; the API is never scraped from `netsh`.
//!
//! Each operation maps to a `wlanapi` sequence:
//! - construct: `WlanOpenHandle`, holding the handle for the lifetime of this struct and releasing
//!   it with `WlanCloseHandle` on `Drop`.
//! - `interfaces`: `WlanEnumInterfaces`.
//! - `scan`: `WlanScan`, await the scan-complete notification, then `WlanGetAvailableNetworkList`.
//! - `connect`: stage a profile with `WlanSetProfile`, then `WlanConnect` (connect is profile-based,
//!   so it is always two calls, not one). It initiates association and returns; the caller polls
//!   `status` for the outcome.
//! - `disconnect`: `WlanDisconnect`.
//! - `status`: `WlanQueryInterface(wlan_intf_opcode_current_connection)`.
//!
//! `wlanapi` is synchronous and callback-driven. The async surface is preserved by awaiting a
//! notification rather than blocking the executor, and the notification callback runs on an OS
//! thread, so it marshals onto a channel and touches no executor-local state. Calls that allocate
//! output buffers are each paired with `WlanFreeMemory`. The client handle is documented as usable
//! from any thread, so it is wrapped to make the backend `Send`/`Sync`.

pub(crate) mod connectivity;
mod conv;
mod ipconfig;
mod profile;
mod radio;

use crate::interface::WifiBackend;
use crate::types::*;
use conv::*;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use windows::Foundation::EventRegistrationToken;
use windows::Win32::Foundation::{BOOL, HANDLE};
use windows::Win32::NetworkManagement::WiFi::*;
use windows::core::{GUID, HSTRING, PCWSTR, PWSTR};

/// The `wlanapi` client handle. The Native WiFi client handle is thread-safe, so the wrapper
/// asserts `Send`/`Sync` to let the backend cross threads under a multi-threaded runtime.
struct Handle(HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

/// A live event subscription: its own dedicated `wlanapi` handle plus the boxed sender handed to
/// the OS callback as context, and the WinRT connectivity-change registration token (when that
/// best-effort registration succeeded). Held by the backend; dropping it stops every source and
/// frees the sender. Kept separate from the main handle so `scan`'s temporary notification
/// registration never clobbers the subscription.
struct EventReg {
    handle: HANDLE,
    sender: *mut UnboundedSender<WifiEvent>,
    net_token: Option<EventRegistrationToken>,
}
unsafe impl Send for EventReg {}

impl Drop for EventReg {
    fn drop(&mut self) {
        // Remove the WinRT connectivity handler first; its captured sender clone is released here.
        if let Some(token) = self.net_token.take() {
            connectivity::remove_status_changed(token);
        }
        unsafe {
            // Closing the handle tears down its notification registration, so no further callback
            // can fire; only then is it safe to free the sender the callback referenced.
            WlanCloseHandle(self.handle, None);
            drop(Box::from_raw(self.sender));
        }
    }
}

/// Scoped registration for awaiting a single connect. Its dedicated handle receives the
/// connection-complete notification (ACM notifications reach every registered handle, regardless of
/// which one issued the connect); dropping it closes the handle and frees the reason-code sender.
struct ConnectReg {
    handle: HANDLE,
    sender: *mut mpsc::Sender<u32>,
}
unsafe impl Send for ConnectReg {}

impl Drop for ConnectReg {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.handle, None);
            drop(Box::from_raw(self.sender));
        }
    }
}

pub struct WindowsWifi {
    handle: Handle,
    events: Mutex<Option<EventReg>>,
}

impl WindowsWifi {
    pub fn new() -> Result<Self> {
        let mut handle = HANDLE::default();
        let mut negotiated = 0u32;
        // Client version 2 is the Vista+ Native WiFi surface this crate targets.
        let ret = unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut handle) };
        check("WlanOpenHandle", ret)?;
        Ok(Self { handle: Handle(handle), events: Mutex::new(None) })
    }

    fn h(&self) -> HANDLE {
        self.handle.0
    }

    /// The GUID of the interface the facade operates on — the first enumerated wireless adapter.
    /// Errors with `NotFound` when the machine has no Wi-Fi interface.
    fn primary_guid(&self) -> Result<GUID> {
        self.enumerate()?
            .into_iter()
            .next()
            .map(|(guid, _, _)| guid)
            .ok_or(WifiError::NotFound)
    }

    /// Raw interface enumeration shared by `interfaces` and `primary_guid`.
    fn enumerate(&self) -> Result<Vec<(GUID, String, ConnectionState)>> {
        let mut list_ptr: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        let ret = unsafe { WlanEnumInterfaces(self.h(), None, &mut list_ptr) };
        check("WlanEnumInterfaces", ret)?;
        if list_ptr.is_null() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        unsafe {
            let list = &*list_ptr;
            let items = std::slice::from_raw_parts(
                list.InterfaceInfo.as_ptr(),
                list.dwNumberOfItems as usize,
            );
            for info in items {
                out.push((
                    info.InterfaceGuid,
                    wide_to_string(&info.strInterfaceDescription),
                    state_from(info.isState),
                ));
            }
            WlanFreeMemory(list_ptr as *const c_void);
        }
        Ok(out)
    }
}

impl Drop for WindowsWifi {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.h(), None);
        }
    }
}

impl WifiBackend for WindowsWifi {
    async fn interfaces(&self) -> Result<Vec<Interface>> {
        Ok(self
            .enumerate()?
            .into_iter()
            .map(|(guid, description, state)| Interface {
                id: guid_to_string(&guid),
                description,
                state,
            })
            .collect())
    }

    async fn scan(&self) -> Result<Vec<Network>> {
        let guid = self.primary_guid()?;

        // Register for ACM notifications, keying a channel the scan-complete callback signals.
        // The boxed sender is handed to the OS as the callback context and reclaimed below.
        let (tx, rx) = mpsc::channel::<()>();
        let ctx = Box::into_raw(Box::new(tx)) as *const c_void;
        let reg = unsafe {
            WlanRegisterNotification(
                self.h(),
                WLAN_NOTIFICATION_SOURCE_ACM,
                BOOL(0),
                Some(scan_callback),
                Some(ctx),
                None,
                None,
            )
        };
        if let Err(e) = check("WlanRegisterNotification", reg) {
            unsafe { drop(Box::from_raw(ctx as *mut mpsc::Sender<()>)) };
            return Err(e);
        }

        let scan_ret = unsafe { WlanScan(self.h(), &guid, None, None, None) };

        // Wait for scan-complete (or fail) without blocking the executor; proceed on timeout so a
        // missed notification still returns whatever the OS already has cached.
        if scan_ret == 0 {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
            loop {
                match rx.try_recv() {
                    Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => {
                        if tokio::time::Instant::now() >= deadline {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }

        // Tear down the registration before dropping the sender it points at.
        unsafe {
            WlanRegisterNotification(
                self.h(),
                WLAN_NOTIFICATION_SOURCE_NONE,
                BOOL(0),
                None,
                None,
                None,
                None,
            );
            drop(Box::from_raw(ctx as *mut mpsc::Sender<()>));
        }

        check("WlanScan", scan_ret)?;
        self.read_available_networks(&guid)
    }

    async fn available_networks(&self) -> Result<Vec<Network>> {
        let guid = self.primary_guid()?;
        self.read_available_networks(&guid)
    }

    async fn request_scan(&self) -> Result<()> {
        let guid = self.primary_guid()?;
        // Fire-and-forget: no notification registration, no wait. The OS delivers scan-complete to
        // any handle registered via `subscribe`, where it surfaces as `WifiEvent::ScanComplete`.
        check("WlanScan", unsafe { WlanScan(self.h(), &guid, None, None, None) })
    }

    async fn connect(&self, req: &ConnectRequest) -> Result<()> {
        if matches!(req.credentials, Credentials::Enterprise { .. }) {
            return Err(WifiError::Unimplemented("windows: enterprise connect"));
        }
        let guid = self.primary_guid()?;

        // Stage a profile (overwriting any prior one for this SSID), unless asked to use the
        // existing stored profile — in which case the credentials are left untouched and only the
        // connect is issued. Connect is always by profile name.
        if !matches!(req.credentials, Credentials::Saved) {
            // A WPA3-SAE-only AP rejects a WPA2PSK profile, so the PSK profile's auth type is
            // chosen from the target's real auth algorithm (read from the cached scan list, no
            // forced rescan). Unknown / not-yet-scanned falls back to the WPA2PSK default.
            let wpa3 = self.auth_algo_for(&guid, &req.ssid) == Some(DOT11_AUTH_SAE);
            let xml = HSTRING::from(profile::profile_xml(&req.ssid, &req.credentials, wpa3));
            let mut reason = 0u32;
            let set = unsafe {
                WlanSetProfile(
                    self.h(),
                    &guid,
                    0,
                    PCWSTR(xml.as_ptr()),
                    PCWSTR::null(),
                    BOOL(1),
                    None,
                    &mut reason,
                )
            };
            check("WlanSetProfile", set)?;
        }

        // Register for the terminal connection-complete notification on a dedicated handle BEFORE
        // issuing the connect, so a fast result can't be missed. The guard closes the handle (which
        // stops callbacks) and frees the sender on drop, and is `Send` so the future stays
        // spawnable across threads.
        let mut notif = HANDLE::default();
        let mut negotiated = 0u32;
        check("WlanOpenHandle(connect)", unsafe {
            WlanOpenHandle(2, None, &mut negotiated, &mut notif)
        })?;
        let (tx, rx) = mpsc::channel::<u32>();
        let sender = Box::into_raw(Box::new(tx));
        let reg = unsafe {
            WlanRegisterNotification(
                notif,
                WLAN_NOTIFICATION_SOURCE_ACM,
                BOOL(0),
                Some(connect_callback),
                Some(sender as *const c_void),
                None,
                None,
            )
        };
        let _guard = ConnectReg { handle: notif, sender };
        check("WlanRegisterNotification(connect)", reg)?;

        // Issue the connect. Scope the profile-name/params (raw pointers) so they drop before the
        // await point and don't make the future non-Send.
        {
            let profile_name = HSTRING::from(req.ssid.as_str());
            let params = WLAN_CONNECTION_PARAMETERS {
                wlanConnectionMode: wlan_connection_mode_profile,
                strProfile: PCWSTR(profile_name.as_ptr()),
                pDot11Ssid: ptr::null_mut(),
                pDesiredBssidList: ptr::null_mut(),
                dot11BssType: dot11_BSS_type_infrastructure,
                dwFlags: 0,
            };
            check("WlanConnect", unsafe { WlanConnect(self.h(), &guid, &params, None) })?;
        }

        // Await the terminal outcome or time out. WlanConnect only *initiates*; the reason code is
        // where success/auth-failure is reported, and Windows retries a wrong key for ~25s before
        // giving up, so the timeout is generous.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
        loop {
            match rx.try_recv() {
                Ok(reason) => return map_connect_reason(reason),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(WifiError::OsApi("connect: notification channel closed".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {
                    if tokio::time::Instant::now() >= deadline {
                        return Err(WifiError::OsApi("connect: timed out".into()));
                    }
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
            }
        }
    }

    async fn disconnect(&self) -> Result<()> {
        let guid = self.primary_guid()?;
        let ret = unsafe { WlanDisconnect(self.h(), &guid, None) };
        check("WlanDisconnect", ret)
    }

    async fn status(&self) -> Result<ConnectionStatus> {
        let guid = self.primary_guid()?;
        let mut size = 0u32;
        let mut data: *mut c_void = ptr::null_mut();
        let ret = unsafe {
            WlanQueryInterface(
                self.h(),
                &guid,
                wlan_intf_opcode_current_connection,
                None,
                &mut size,
                &mut data,
                None,
            )
        };
        // Not-connected interfaces return an error for this opcode rather than a struct; report a
        // clean Disconnected instead of surfacing the OS error.
        if ret != 0 || data.is_null() {
            return Ok(ConnectionStatus {
                state: ConnectionState::Disconnected,
                ssid: None,
                signal: None,
            });
        }

        let status = unsafe {
            let attrs = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
            let assoc = &attrs.wlanAssociationAttributes;
            let state = state_from(attrs.isState);
            let ssid = ssid_to_string(&assoc.dot11Ssid);
            let out = ConnectionStatus {
                state,
                ssid: (!ssid.is_empty()).then_some(ssid),
                signal: Some(assoc.wlanSignalQuality as u8),
            };
            WlanFreeMemory(data as *const c_void);
            out
        };
        Ok(status)
    }

    async fn ip_config(&self) -> Result<IpConfig> {
        let guid = self.primary_guid()?;
        ipconfig::read_ip_config(&guid)
    }

    async fn connectivity(&self) -> Result<Connectivity> {
        connectivity::current()
    }

    async fn saved_networks(&self) -> Result<Vec<SavedNetwork>> {
        let guid = self.primary_guid()?;
        let mut list_ptr: *mut WLAN_PROFILE_INFO_LIST = ptr::null_mut();
        let ret = unsafe { WlanGetProfileList(self.h(), &guid, None, &mut list_ptr) };
        check("WlanGetProfileList", ret)?;
        if list_ptr.is_null() {
            return Ok(Vec::new());
        }

        // Profile list order is the OS's connection preference (highest first); expose it as a
        // descending priority so callers can show/honour auto-connect order.
        let names: Vec<String> = unsafe {
            let list = &*list_ptr;
            let items = std::slice::from_raw_parts(
                list.ProfileInfo.as_ptr(),
                list.dwNumberOfItems as usize,
            );
            let names = items
                .iter()
                .map(|p| wide_to_string(&p.strProfileName))
                .collect();
            WlanFreeMemory(list_ptr as *const c_void);
            names
        };

        let count = names.len() as i32;
        Ok(names
            .into_iter()
            .enumerate()
            .map(|(i, ssid)| SavedNetwork {
                security: self.profile_security(&guid, &ssid),
                ssid,
                priority: Some(count - i as i32),
            })
            .collect())
    }

    async fn forget(&self, ssid: &str) -> Result<()> {
        let guid = self.primary_guid()?;
        let name = HSTRING::from(ssid);
        let ret = unsafe { WlanDeleteProfile(self.h(), &guid, PCWSTR(name.as_ptr()), None) };
        // ERROR_NOT_FOUND (1168): no stored profile for this SSID.
        if ret == 1168 {
            return Err(WifiError::NotFound);
        }
        check("WlanDeleteProfile", ret)
    }

    fn subscribe(&self) -> Result<UnboundedReceiver<WifiEvent>> {
        // Dedicated handle so this registration is independent of the main handle (which `scan`
        // briefly registers a notification on).
        let mut handle = HANDLE::default();
        let mut negotiated = 0u32;
        check("WlanOpenHandle(events)", unsafe {
            WlanOpenHandle(2, None, &mut negotiated, &mut handle)
        })?;

        let (tx, rx) = unbounded_channel::<WifiEvent>();

        // Emit the current connectivity immediately so a new subscriber sees it without waiting for
        // the first change; the WinRT handler below gets its own clone for subsequent updates.
        let net_tx = tx.clone();
        if let Ok(c) = connectivity::current() {
            let _ = tx.send(WifiEvent::Connectivity(c));
        }

        let sender = Box::into_raw(Box::new(tx));
        let reg = unsafe {
            WlanRegisterNotification(
                handle,
                WLAN_NOTIFICATION_SOURCE_ACM | WLAN_NOTIFICATION_SOURCE_MSM,
                BOOL(0),
                Some(event_callback),
                Some(sender as *const c_void),
                None,
                None,
            )
        };
        if let Err(e) = check("WlanRegisterNotification", reg) {
            unsafe {
                WlanCloseHandle(handle, None);
                drop(Box::from_raw(sender));
            }
            return Err(e);
        }

        // Push OS internet-reachability changes onto the same stream. Best-effort: if the WinRT
        // registration fails, Wi-Fi link events still flow and `net_token` is simply `None`.
        let net_token =
            connectivity::register_status_changed(move |c| {
                let _ = net_tx.send(WifiEvent::Connectivity(c));
            })
            .ok();

        // Storing the new reg drops any previous one (closing its handle, freeing its sender), so a
        // re-subscribe cleanly replaces the old stream. A poisoned lock can't lose the reg.
        *self.events.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(EventReg { handle, sender, net_token });
        Ok(rx)
    }

    async fn is_enabled(&self) -> Result<bool> {
        radio::is_enabled()
    }

    async fn set_enabled(&self, on: bool) -> Result<()> {
        radio::set_enabled(on)
    }
}

impl WindowsWifi {
    /// Read the current available-network list for `guid` and fold it into neutral `Network`s,
    /// de-duplicating by SSID and keeping the strongest signal (the list reports one entry per
    /// security variant, which would otherwise surface the same SSID several times).
    fn read_available_networks(&self, guid: &GUID) -> Result<Vec<Network>> {
        let mut list_ptr: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
        let ret =
            unsafe { WlanGetAvailableNetworkList(self.h(), guid, 0, None, &mut list_ptr) };
        check("WlanGetAvailableNetworkList", ret)?;
        if list_ptr.is_null() {
            return Ok(Vec::new());
        }

        let mut best: HashMap<String, Network> = HashMap::new();
        unsafe {
            let list = &*list_ptr;
            let items =
                std::slice::from_raw_parts(list.Network.as_ptr(), list.dwNumberOfItems as usize);
            for net in items {
                let ssid = ssid_to_string(&net.dot11Ssid);
                if ssid.is_empty() {
                    continue; // hidden / unnamed APs have no actionable SSID here
                }
                let signal = net.wlanSignalQuality as u8;
                let candidate = Network {
                    ssid: ssid.clone(),
                    bssid: None,
                    security: security_from(
                        net.bSecurityEnabled.as_bool(),
                        net.dot11DefaultAuthAlgorithm.0,
                    ),
                    signal: Some(signal),
                    frequency_mhz: None,
                    saved: (net.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE) != 0,
                };
                best.entry(ssid)
                    .and_modify(|existing| {
                        if signal > existing.signal.unwrap_or(0) {
                            *existing = candidate.clone();
                        }
                    })
                    .or_insert(candidate);
            }
            WlanFreeMemory(list_ptr as *const c_void);
        }

        Ok(best.into_values().collect())
    }

    /// The default auth algorithm the AP for `ssid` advertises, read from the cached available-
    /// network list (no forced rescan). `None` if the SSID isn't currently visible. Used to pick
    /// the right PSK profile flavour at connect time.
    fn auth_algo_for(&self, guid: &GUID, ssid: &str) -> Option<i32> {
        let mut list_ptr: *mut WLAN_AVAILABLE_NETWORK_LIST = ptr::null_mut();
        let ret = unsafe { WlanGetAvailableNetworkList(self.h(), guid, 0, None, &mut list_ptr) };
        if ret != 0 || list_ptr.is_null() {
            return None;
        }
        let found = unsafe {
            let list = &*list_ptr;
            let items =
                std::slice::from_raw_parts(list.Network.as_ptr(), list.dwNumberOfItems as usize);
            let found = items
                .iter()
                .find(|net| ssid_to_string(&net.dot11Ssid) == ssid)
                .map(|net| net.dot11DefaultAuthAlgorithm.0);
            WlanFreeMemory(list_ptr as *const c_void);
            found
        };
        found
    }

    /// Best-effort security of a stored profile: fetch its XML (`WlanGetProfile`) and read the
    /// `<authentication>` element. `Unknown` if the profile can't be read or parsed.
    fn profile_security(&self, guid: &GUID, name: &str) -> Security {
        let name_h = HSTRING::from(name);
        let mut xml_ptr = PWSTR::null();
        let mut flags = 0u32;
        let mut access = 0u32;
        let ret = unsafe {
            WlanGetProfile(
                self.h(),
                guid,
                PCWSTR(name_h.as_ptr()),
                None,
                &mut xml_ptr,
                Some(&mut flags),
                Some(&mut access),
            )
        };
        if ret != 0 || xml_ptr.is_null() {
            return Security::Unknown;
        }
        unsafe {
            let xml = xml_ptr.to_string().unwrap_or_default();
            WlanFreeMemory(xml_ptr.0 as *const c_void);
            security_from_profile_xml(&xml)
        }
    }
}

/// `DOT11_AUTH_ALGORITHM` value for WPA3-SAE; an AP advertising it rejects a WPA2PSK profile.
const DOT11_AUTH_SAE: i32 = 9;

/// ACM notification callback. Runs on an OS thread; it only inspects the notification code and,
/// on scan completion or failure, signals the channel whose sender it was handed as context.
unsafe extern "system" fn scan_callback(data: *mut L2_NOTIFICATION_DATA, ctx: *mut c_void) {
    if data.is_null() || ctx.is_null() {
        return;
    }
    let data = unsafe { &*data };
    if data.NotificationSource != WLAN_NOTIFICATION_SOURCE_ACM {
        return;
    }
    // wlan_notification_acm_scan_complete / _scan_fail are the two terminal scan outcomes.
    let code = data.NotificationCode as i32;
    if code == wlan_notification_acm_scan_complete.0 || code == wlan_notification_acm_scan_fail.0 {
        let tx = unsafe { &*(ctx as *const mpsc::Sender<()>) };
        let _ = tx.send(());
    }
}

/// Subscription notification callback. Runs on an OS thread; translates ACM (connection lifecycle,
/// scan) and MSM (signal) notifications into `WifiEvent`s and enqueues them on the unbounded sender
/// it was handed as context. Unrecognized notifications are ignored.
unsafe extern "system" fn event_callback(data: *mut L2_NOTIFICATION_DATA, ctx: *mut c_void) {
    if data.is_null() || ctx.is_null() {
        return;
    }
    let data = unsafe { &*data };
    let event = match data.NotificationSource {
        WLAN_NOTIFICATION_SOURCE_ACM => match data.NotificationCode as i32 {
            c if c == wlan_notification_acm_scan_complete.0 => Some(WifiEvent::ScanComplete),
            c if c == wlan_notification_acm_connection_start.0 => {
                Some(WifiEvent::StateChanged(ConnectionState::Connecting))
            }
            c if c == wlan_notification_acm_connection_complete.0 => {
                Some(WifiEvent::StateChanged(ConnectionState::Connected))
            }
            c if c == wlan_notification_acm_connection_attempt_fail.0 => {
                Some(WifiEvent::StateChanged(ConnectionState::Failed))
            }
            c if c == wlan_notification_acm_disconnecting.0 => {
                Some(WifiEvent::StateChanged(ConnectionState::Disconnecting))
            }
            c if c == wlan_notification_acm_disconnected.0 => {
                Some(WifiEvent::StateChanged(ConnectionState::Disconnected))
            }
            _ => None,
        },
        WLAN_NOTIFICATION_SOURCE_MSM => match data.NotificationCode as i32 {
            c if c == wlan_notification_msm_signal_quality_change.0 => {
                // pData is a WLAN_SIGNAL_QUALITY (ULONG, 0..100).
                if !data.pData.is_null() && data.dwDataSize as usize >= size_of::<u32>() {
                    let quality = unsafe { *(data.pData as *const u32) };
                    Some(WifiEvent::SignalChanged((quality.min(100)) as u8))
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(event) = event {
        let tx = unsafe { &*(ctx as *const UnboundedSender<WifiEvent>) };
        let _ = tx.send(event);
    }
}

/// Connect-await callback. Fires on every ACM notification and forwards a `wlanReasonCode` to the
/// waiting connect for the two terminal cases:
/// - `connection_complete` — the success/give-up signal (reason 0 == connected).
/// - `connection_attempt_fail` with a security-range reason — a definitive credential failure (a
///   wrong key never recovers on retry). Transient attempt-fails are ignored so a connect that
///   succeeds on a later retry isn't reported as failed.
unsafe extern "system" fn connect_callback(data: *mut L2_NOTIFICATION_DATA, ctx: *mut c_void) {
    if data.is_null() || ctx.is_null() {
        return;
    }
    let data = unsafe { &*data };
    if data.NotificationSource != WLAN_NOTIFICATION_SOURCE_ACM {
        return;
    }
    let code = data.NotificationCode as i32;

    let reason = if !data.pData.is_null()
        && data.dwDataSize as usize >= size_of::<WLAN_CONNECTION_NOTIFICATION_DATA>()
    {
        unsafe { (*(data.pData as *const WLAN_CONNECTION_NOTIFICATION_DATA)).wlanReasonCode }
    } else {
        0
    };

    let is_complete = code == wlan_notification_acm_connection_complete.0;
    let is_attempt_fail = code == wlan_notification_acm_connection_attempt_fail.0;
    if !is_complete && !is_attempt_fail {
        return;
    }

    if is_complete || is_security_reason(reason) {
        let tx = unsafe { &*(ctx as *const mpsc::Sender<u32>) };
        let _ = tx.send(reason);
    }
}

/// WLAN reason codes in the MSMSEC band (`0x40000..0x50000`) are security/credential failures — a
/// wrong key reports here (observed: `0x48014`). The lower MSM band (`0x30000`) is general
/// connection failure, not specifically auth.
fn is_security_reason(reason: u32) -> bool {
    (0x0004_0000..0x0005_0000).contains(&reason)
}

/// Map a connection-complete `wlanReasonCode` to a result. The MSMSEC range (`0x30000..0x40000`)
/// is the security/credential failure band — a wrong key lands here — and maps to `AuthFailed`;
/// anything else non-zero is surfaced with its raw code.
fn map_connect_reason(reason: u32) -> Result<()> {
    if reason == 0 {
        Ok(())
    } else if is_security_reason(reason) {
        Err(WifiError::AuthFailed)
    } else {
        Err(WifiError::OsApi(format!("connect failed: WLAN reason 0x{reason:X}")))
    }
}
