# System patterns

- Single Rust binary (`keeld` via `cargo run` package `keel`) serves API + static UI.
- Storage is a trait: `LinuxInventory` (lsblk) vs `MockInventory` (Windows/dev).
- JSON API under `/api/*`. UI is static files from `web/`.
- Notes persist in `data/notes.json` (`KEEL_DATA` to override). Pool plans persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel also writes `smb-shares.conf` and serves `/var/lib/keel/files` over SMB. Proxmox tickets for Check files stay in RAM only.
- Look around (`POST /api/look`) knocks on a single host: 8006, 443, 80, 445, 2049. No subnet scan. Typed hosts are remembered in `data/remote.json` until deleted. Sign-in (`POST /api/look/login`) lists Proxmox/TrueNAS storage and file apps. `GET /api/look/files` + `POST /api/look/browse` list OpenCloud/WebDAV, Windows folders, or Proxmox storage content. Password is not written to disk.
- Remote install is a **separate control-plane tool** (`keelctl`) on the admin PC. It never assumes the admin OS equals the NAS OS.
- Networking: bind a chosen LAN interface; show IP:port and UNC. Overlay is the away path for the **same** shares — show that address next to the LAN one. Do not depend on mDNS.
- Overlay is provider-neutral in Keel. Headscale is self-hosted-first; Tailscale is a compatible client/control option. The app asks Keel for overlay status and addresses rather than hardcoding a provider.
- Versioned base configuration is loaded from `KEEL_DATA/config.json`; writes use a temporary file plus rename. Overlay mutation is intentionally not exposed until admin authentication exists.
- Transitional admin boundary: mutation routes use bearer auth from `KEEL_ADMIN_TOKEN`; loopback development may omit it, but non-loopback startup fails closed without it.
- Guide is static HTML next to the dashboard. `/repair.html` is local issues + playbook Fixes.
- UI shell: `html/body` `overflow: hidden`; `main` is a column; one `.scroll` (or `.table-wrap.scroll`) per page. Sidebar does not scroll. Interaction rules in [ui.md](ui.md) (from Cerebro). Not a tab per subsystem.

## Objects

- Disk
- Pool (plan persisted)
- Share (SMB on Linux; Windows copy cannot serve folders)
- Note (scratch pad)
- System snapshot (CPU, RAM, process RSS)
