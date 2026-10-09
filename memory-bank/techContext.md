# Tech context

- Language: Rust 2021, axum 0.8, tokio, sysinfo
- UI: static HTML/CSS/JS, fetch()
- Host: Linux production; Windows mock inventory
- Port: 9478 (`KEEL_PORT` to override); bind: `127.0.0.1` by default (`KEEL_BIND` to override)

## Install / intranet

- Target already has Windows or Linux
- Linux NAS: Debian **minbase + named packages only** (see os.md). Not a full Debian desktop/server role.
- Linux NAS build ≠ Windows admin build
- See intranet.md for VLAN, Proxmox bridge, firewall, discovery
- Overlay (Tailscale package today) is required for files away from home; Samba must listen on that path. Brand can change.
- Linux Keel Live includes Samba. House-network File Explorer path is `\\ip\files` with name `keel` and password `keel` until real sign-in exists.

## Constraints

- Do not add Electron, Node, or a desktop session for admin
- Keep the binary one process
- Prefer parsing existing tools (`lsblk`) over reimplementing block devices in v0
