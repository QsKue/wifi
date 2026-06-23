//! Compile-time platform selection. Exactly one backend is aliased to `Backend` for the current
//! target, and the [`crate::WiFi`] facade constructs and delegates to it. Binding a single
//! concrete type per build keeps dispatch static and avoids any runtime backend indirection.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsWifi as Backend;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxWifi as Backend;

#[cfg(not(any(windows, target_os = "linux")))]
mod dummy;
#[cfg(not(any(windows, target_os = "linux")))]
pub use dummy::DummyWifi as Backend;
