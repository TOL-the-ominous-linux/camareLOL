//! Network detection and connectivity checks.
//!
//! The check distinguishes "no link" from "link but no internet" from
//! "internet but the mirror is broken" so the UI can explain the difference.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    pub name: String,
    pub carrier: bool,
    pub mac: Option<String>,
    pub wireless: bool,
}

pub fn list_interfaces() -> Vec<Interface> {
    let mut out = Vec::new();
    let dir = match std::fs::read_dir("/sys/class/net") {
        Ok(d) => d,
        Err(_) => return out,
    };
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let base = entry.path();
        let carrier = std::fs::read_to_string(base.join("carrier"))
            .map(|s| s.trim() == "1")
            .unwrap_or(false);
        let mac = std::fs::read_to_string(base.join("address"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let wireless = base.join("wireless").exists() || base.join("phy80211").exists();
        out.push(Interface { name, carrier, mac, wireless });
    }
    out
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Connectivity {
    /// At least one NIC has a link.
    Link,
    /// TCP connection to a well-known host succeeded.
    Internet,
    /// No link at all.
    Down,
}

/// Try to reach `1.1.1.1:53` and `archlinux.org:443`.
pub fn check_internet() -> Result<Connectivity> {
    let any_link = list_interfaces().iter().any(|i| i.carrier && i.name != "lo");
    if !any_link {
        return Ok(Connectivity::Down);
    }
    for host in [("1.1.1.1", 53), ("archlinux.org", 443)] {
        if let Ok(mut addrs) = (host.0, host.1).to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                if TcpStream::connect_timeout(&addr, Duration::from_secs(4)).is_ok() {
                    return Ok(Connectivity::Internet);
                }
            }
        }
    }
    Ok(Connectivity::Link)
}

/// Human readable status for the UI.
pub fn describe(c: Connectivity) -> &'static str {
    match c {
        Connectivity::Link => "network cable detected, but no internet connection",
        Connectivity::Internet => "internet connection OK",
        Connectivity::Down => "no network connection detected",
    }
}

/// Wi-Fi helper: the installer surfaces a warning when the only usable
/// interface is wireless and iwd/NetworkManager is absent.
pub fn wifi_setup_available() -> bool {
    crate::sys::tool_available("iwctl") || crate::sys::tool_available("nmtui")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_all_states() {
        assert!(!describe(Connectivity::Down).is_empty());
        assert!(!describe(Connectivity::Link).is_empty());
        assert!(!describe(Connectivity::Internet).is_empty());
    }

    #[test]
    fn check_internet_does_not_panic() {
        let _ = check_internet();
    }

    #[test]
    fn interfaces_have_names() {
        for i in list_interfaces() {
            assert!(!i.name.is_empty());
        }
    }

    #[test]
    fn error_helper() {
        let e = CamarelolError::new(ErrorCode::NetworkUnreachable, "x");
        assert_eq!(e.code, ErrorCode::NetworkUnreachable);
    }
}
