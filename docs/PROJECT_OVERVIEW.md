# Project Overview

## What it is
`wifi` is a Rust library for **programmatically controlling a machine's Wi-Fi**: list adapters,
scan for networks, join and leave them, read connection status, and manage the OS's saved
profiles. It is a building block, not an application — host programs embed it.

## Why it exists
The workspace previously vendored [`wifi-rs`](../../wifi-rs), which drives Wi-Fi by **shelling out
to CLI tools** (`netsh` on Windows, `nmcli` on Linux) and parsing their text. That is brittle
(output format drift, localization, missing exit codes), slow, and can't deliver events. `wifi`
does the same job through each OS's **native programming interface**, so it's robust, fast, and
able to stream connection events.

## Who uses it
- **qjay** — the Q-Lab player appliance. Runs on Windows (development) and Linux (the OrangePi/ARM
  device); macOS is a future target. Needs reliable headless connect/disconnect and status.
- **qshell** — Windows-only. Needs the same station control on the desktop.

Because qjay ships on minimal Linux images, the Linux backend is designed to allow a second
daemon (`wpa_supplicant`) behind the same trait later, even though NetworkManager is the starting
point. See [ROADMAP.md](ROADMAP.md).

## Scope
**In scope** (phased — see the roadmap): station operations (scan/connect/disconnect/status),
saved-profile management, a live connection-event stream, and eventually hotspot/AP mode and
WPA-Enterprise.

**Out of scope:** being a full network-config manager (routing, VPN, Ethernet), a UI, or a daemon.
It controls Wi-Fi and reports state; policy lives in the host program.

For *how* this is built (modules, trait, backends), see [ARCHITECTURE.md](ARCHITECTURE.md) — keep
implementation rules out of this file.
