//! IP-layer interface details via the IP Helper API (`GetAdaptersAddresses`). This is a separate
//! API from `wlanapi` — Wi-Fi control is link-layer, while MAC / IP / gateway / DNS live in the
//! networking stack. Kept in its own module so the IP-Helper FFI doesn't mix with the radio code.

use super::conv::{check, guid_to_string};
use crate::types::*;
use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6};
use windows::core::GUID;

/// Read the IP configuration of the adapter identified by `guid`. `GetAdaptersAddresses` returns a
/// linked list of all adapters; the one whose `AdapterName` (a GUID string) matches is mapped into
/// [`IpConfig`]. `NotFound` if the adapter isn't present.
pub(crate) fn read_ip_config(guid: &GUID) -> Result<IpConfig> {
    // INCLUDE_GATEWAYS adds the gateway list (off by default); skip the address classes we don't use.
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;

    // First call sizes the buffer (returns ERROR_BUFFER_OVERFLOW); the second fills it. The buffer
    // is a Vec<u64> so it satisfies the struct's 8-byte alignment (a Vec<u8> would not).
    let mut size = 0u32;
    unsafe { GetAdaptersAddresses(0, flags, None, None, &mut size) };
    if size == 0 {
        return Err(WifiError::OsApi("GetAdaptersAddresses returned zero size".into()));
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    let base = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
    let ret = unsafe { GetAdaptersAddresses(0, flags, None, Some(base), &mut size) };
    check("GetAdaptersAddresses", ret)?;

    let want = guid_to_string(guid).to_uppercase();
    unsafe {
        let mut cur = base;
        while !cur.is_null() {
            let adapter = &*cur;
            if adapter.AdapterName.to_string().unwrap_or_default().to_uppercase() == want {
                return Ok(build(adapter));
            }
            cur = adapter.Next;
        }
    }
    Err(WifiError::NotFound)
}

unsafe fn build(adapter: &IP_ADAPTER_ADDRESSES_LH) -> IpConfig {
    let mac = {
        let n = adapter.PhysicalAddressLength as usize;
        (n >= 6).then(|| {
            adapter.PhysicalAddress[..6]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(":")
        })
    };

    let (mut ipv4, mut ipv6) = (Vec::new(), Vec::new());
    let mut ua = adapter.FirstUnicastAddress;
    while !ua.is_null() {
        let entry = unsafe { &*ua };
        if let Some(ip) = unsafe { sockaddr_to_ip(entry.Address.lpSockaddr) } {
            if ip.contains(':') { ipv6.push(ip) } else { ipv4.push(ip) }
        }
        ua = entry.Next;
    }

    let mut gateway = None;
    let mut g = adapter.FirstGatewayAddress;
    while !g.is_null() {
        let entry = unsafe { &*g };
        if let Some(ip) = unsafe { sockaddr_to_ip(entry.Address.lpSockaddr) } {
            gateway = Some(ip);
            break;
        }
        g = entry.Next;
    }

    let mut dns = Vec::new();
    let mut d = adapter.FirstDnsServerAddress;
    while !d.is_null() {
        let entry = unsafe { &*d };
        if let Some(ip) = unsafe { sockaddr_to_ip(entry.Address.lpSockaddr) } {
            dns.push(ip);
        }
        d = entry.Next;
    }

    IpConfig { mac, ipv4, ipv6, gateway, dns }
}

/// Render a `SOCKADDR` as a textual IPv4/IPv6 address, or `None` for other families.
unsafe fn sockaddr_to_ip(sa: *const SOCKADDR) -> Option<String> {
    if sa.is_null() {
        return None;
    }
    let family = unsafe { (*sa).sa_family };
    if family == AF_INET {
        let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
        let o = unsafe { sin.sin_addr.S_un.S_un_b };
        Some(Ipv4Addr::new(o.s_b1, o.s_b2, o.s_b3, o.s_b4).to_string())
    } else if family == AF_INET6 {
        let sin6 = unsafe { &*(sa as *const SOCKADDR_IN6) };
        let bytes = unsafe { sin6.sin6_addr.u.Byte };
        Some(Ipv6Addr::from(bytes).to_string())
    } else {
        None
    }
}
