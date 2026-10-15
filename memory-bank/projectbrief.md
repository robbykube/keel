# Project brief

**keel** is an **appliance OS**: Linux you install on the NAS box, with a browser GUI, the rok catalog baked in, and opinionated idle/power. Not a desktop (Omarchy), not a hypervisor (Proxmox).

What exists in this repo today is the control plane (daemon + GUI) and a Debian live image that can install Keel on a whole disk. The installed box at home is still that appliance. PXE and a second root slot are not built.

Two ways onto a machine:

- **OS install** (the product): image/ISO, box becomes keel.
- **Push onto existing Linux** (bootstrap): SSH from Windows, for a box that already has Debian/Ubuntu.

Windows is for building the GUI and for `keelctl`, not the NAS OS.

## Goals

- Proxmox-like admin: browser, dense, few clicks to a working share
- Same share at home and away: Tailscale (or successor overlay) is the file path, not an optional VPN extra
- Faster and leaner than TrueNAS/Unraid for the core path (inventory, pool, SMB)
- Opinionated defaults (Omarchy idea) without Omarchy’s desktop/TUI posture

## File manifest

- productContext.md
- systemPatterns.md
- techContext.md
- activeContext.md
- troubleshooting.md
- intranet.md (install onto existing OS; LAN/VLAN/Proxmox failure modes)
- lab-rok.md (this homelab’s Proxmox inventory)
- catalog-rok.md (ship-with apps, including rok’s stopped LXCs)
- guide-and-power.md (Guide, idle floor, advisor)
- os.md (Debian appliance layers; the live image installs the box)
- health-and-repair.md (local issues + one-click Fix; AI later)
- setup-and-report.md (few-question setup)
- storage-backends.md (keep data on Proxmox/TrueNAS; keel is the rest)
- words.md (plain language for the GUI)
- ui.md (Cerebro interaction/layout intents; Keel keeps its own look)
- updates.md (signed program updates; whole-disk A/B still a design)
- envisaged.md (later pillars, not the current slice)
- os-strategy.md (Debian image, not a from-scratch OS)
- size.md (what a later cut might remove)

The product is the OS. This repo’s image installs it. Existing Linux is a bootstrap path, not the end state.

## Out of scope (v0)

- Full ZFS feature parity
- Third-party plugin bazaar (we ship a fixed rok-inspired catalog instead)
- Desktop environment on the NAS
