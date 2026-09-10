# Active context

## Focus

GUI shell: **window is the frame**. Wide pages use two panes so they fill the window (Guide words | rest; Pools look/sign-in | add/list).

rok catalog is **ship-with**. Idle = Tailscale floor (files + GUI from away). Active = demand, not all-on.

- `cargo run` serves GUI on port 9478
- Mock disks on non-Linux
- Pools/shares: pools now persist in `data/pools.json`. Shares still in-memory. Notes survive restart; Look around hosts are remembered until deleted
- After Proxmox sign-in, a session stays in RAM so Pools can list OpenCloud / storage content (Check files). Password is not written to disk.

## What to do on this Windows copy

Click around. Write ideas in Notes. Sign in on Pools to list files on rok (OpenCloud), without wiping anything. Do not wipe rok. Real mounts wait for Linux.

## Next

- On Linux, mount foreign NFS/SMB and run Samba on LAN **and** the overlay
- Show overlay address (Tailscale IP / MagicDNS) next to each share, same name as the house path
- Optional later: remember a Proxmox API token instead of retyping the password
- Wire idle sensors for the advisor (disk/CPU)
- Package the rok catalog (including 101, 106, 107, 109)

## Decisions

- Product name: keel
- GUI not TUI
- Rust single binary for speed/RSS
- Product is an appliance OS: Debian minbase + named packages + keel. Not full Debian.
- Discovery is optional; IP:port is the contract. Look around is typed-host, not mDNS/LAN sweep.
- Linux is the real pool/SMB stack; Windows host is a distinct backend
- `keelctl` on the admin PC pushes the matching binary over SSH/WinRM
- Explanations live in the GUI (Guide + Why this?), not only in markdown
- Overlay (Tailscale today) is how files work away from home. Same share. Never stop it to save watts. Brand can change; the job cannot.
- Power advisor: even while Active, score busy vs parked and explain each save
- Ship the rok LXC set (including currently stopped guests); not a plugin bazaar
- Storage may stay on rok/TrueNAS (foreign mount). keel replaces the rest, not the bits, until we have a dedicated NAS box.
- Admin chrome stays on screen; lists scroll. Same as AI Suite.
- Listing ZFS inside Proxmox/TrueNAS: sign in on Pools. Password stays in the GUI on this PC, never in git or chat. keel stores name + host only.
