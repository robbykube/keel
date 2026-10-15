# System patterns

- Single Rust binary (`keeld` via `cargo run` package `keel`) serves API + static UI.
- Storage is a trait: `LinuxInventory` (lsblk) vs `MockInventory` (Windows/dev).
- JSON API under `/api/*`. UI is static files from `web/`.
- Notes persist in `data/notes.json` (`KEEL_DATA` to override). Pool plans persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel also writes `smb-shares.conf` and serves `/var/lib/keel/files` over SMB. Proxmox tickets for Check files stay in RAM only.
- Disks in this computer: `GET /api/storage` (disks with what they hold, jobs, owner, copy status); admin `POST /api/storage/prepare {path, role, confirm}`, `/owner {name, password}`, `/copy-now`. `src/storage.rs` runs host scripts via systemd-run; `src/copies.rs` is the daily mirror with `Older/<date>/`; `main.rs` `watch_disks` re-applies Samba when a disk comes or goes and starts due copies.
- Look around (`POST /api/look`) knocks on a single host: 8006, 443, 80, 445, 2049. No subnet scan. Typed hosts are remembered in `data/remote.json` until deleted. Sign-in (`POST /api/look/login`) lists Proxmox/TrueNAS storage and file apps. `GET /api/look/files` + `POST /api/look/browse` list OpenCloud/WebDAV, Windows folders, or Proxmox storage content. Password is not written to disk.
- Remote install is a **separate control-plane tool** (`keelctl`) on the admin PC. It never assumes the admin OS equals the NAS OS.
- Networking: bind a chosen LAN interface; show IP:port and UNC. Overlay is the away path for the **same** shares — show that address next to the LAN one. Do not depend on mDNS.
- Overlay is provider-neutral in Keel. Headscale is self-hosted-first; Tailscale is a compatible client/control option. The app asks Keel for overlay status and addresses rather than hardcoding a provider.
- Versioned base configuration is loaded from `KEEL_DATA/config.json`; writes use a temporary file plus rename. Overlay mutation is intentionally not exposed until admin authentication exists.
- Admin boundary: mutation routes accept `Authorization: Bearer` from `KEEL_ADMIN_TOKEN`, or an HttpOnly `keel_session` cookie. `/login.html` can still exchange the token for a one-day session. Another computer can `POST /api/auth/requests`; the Keel screen (loopback only) shows Allow / Not now and that answer starts the session. Non-loopback startup fails closed without a token. Loopback trust (`KEEL_TRUST_LOOPBACK=1`) is the kiosk's own API access, separate from this click.
- Guide is static HTML next to the dashboard. `/repair.html` is local issues + playbook Fixes.
- UI shell: `html/body` `overflow: hidden`; `main` is a column; one `.scroll` (or `.table-wrap.scroll`) per page. Sidebar does not scroll (`aside { overflow: hidden }` on desktop). Everyday rows plus a This computer flyout; Notes at the bottom. Interaction rules in [ui.md](ui.md) (from Cerebro). Not a tab per subsystem.
- Whole-disk install is ISO-only: the sidebar Install item and `POST /api/installer/plan` require a live USB (`KEEL_INSTALLER_ENABLED=1`, helper, and `/run/live/medium/live/filesystem.squashfs`). The installed app does not show Install. `/api/health` `can_install` is how the chrome decides. The helper writes `/run/keel/install.json`; the page polls `GET /api/installer/progress`. Tool output stays in `/run/keel/install.log`.

## Objects

- Disk
- Pool (plan persisted)
- Share (SMB on Linux; Windows copy cannot serve folders)
- Note (scratch pad)
- System snapshot (CPU, RAM, process RSS)
