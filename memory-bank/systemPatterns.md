# System patterns

- Single Rust binary (`keeld` via `cargo run` package `keel`) serves API + static UI.
- Storage is a trait: `LinuxInventory` (lsblk) vs `MockInventory` (Windows/dev).
- JSON API under `/api/*`. UI is static files from `web/`.
- Notes persist in `data/notes.json` (`KEEL_DATA` to override). Pool plans persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel also writes `smb-shares.conf` and serves `/var/lib/keel/files` over SMB. Proxmox tickets for Check files stay in RAM only.
- Disks in this computer: `GET /api/storage` (disks with what they hold, jobs, owner, copy status); admin `POST /api/storage/prepare {path, role, confirm}`, `/owner {name, password}`, `/copy-now`. `src/storage.rs` runs host scripts via systemd-run; `src/copies.rs` is the daily mirror with `Older/<date>/`; `main.rs` `watch_disks` re-applies Samba when a disk comes or goes and starts due copies.
- Look around (`POST /api/look`) knocks on a single host: 8006, 443, 80, 445, 2049. No subnet scan. The computer and what answered are saved in `data/remote.json` until deleted, so Disks shows them again without looking. Sign-in (`POST /api/look/login`) lists Proxmox/TrueNAS storage and file apps. `GET /api/look/files` + `POST /api/look/browse` list OpenCloud/WebDAV, Windows folders, or Proxmox storage content. Password is not written to disk.
- Remote install is a **separate control-plane tool** (`keelctl`) on the admin PC. It never assumes the admin OS equals the NAS OS.
- Networking: bind a chosen LAN interface; show IP:port and UNC. Overlay is the away path for the **same** shares — show that address next to the LAN one. Do not depend on mDNS. Repair “Only the house and Away” (`src/firewall.rs`) drops new incoming connections except loopback, the house /24 on 445 and 9478, and Tailscale (its port, plus 445, 9478, and 443 on `tailscale0`). Away’s “Lock the page” (`src/locked.rs`) serves that same page on 443 with a Tailscale certificate. Keel terminates it. A proxy must not sit in front. Outbound stays open. The flag is `KEEL_DATA/firewall.on`. It is off until that button is pressed.

- Overlay is provider-neutral in Keel. Headscale is self-hosted-first; Tailscale is a compatible client/control option. The app asks Keel for overlay status and addresses rather than hardcoding a provider.
- Versioned base configuration is loaded from `KEEL_DATA/config.json`; writes use a temporary file plus rename. Overlay mutation is intentionally not exposed until admin authentication exists.
- Admin boundary: mutation routes accept `Authorization: Bearer` from `KEEL_ADMIN_TOKEN`, or an HttpOnly `keel_session` cookie. People sign in with a name and password. The password is stored only as a stretched hash in `KEEL_DATA/people.json`. The session records that name for a day, in memory, so a restart asks again. The first person is added on the Keel screen, in Settings. The admin token and Allow on the screen still work, and those sign-ins are recorded without a person’s name. Another computer can `POST /api/auth/requests`; the Keel screen (loopback only) shows Allow / Not now. Non-loopback startup fails closed without a token. Loopback trust (`KEEL_TRUST_LOOPBACK=1`) is the kiosk's own API access, separate from this click.
- Guide is static HTML next to the dashboard. `/repair.html` is local issues + playbook Fixes.
- UI shell: `html/body` `overflow: hidden`; `main` is a column; one `.scroll` (or `.table-wrap.scroll`) per page. Sidebar does not scroll (`aside { overflow: hidden }` on desktop). Everyday rows are Home, Files, Away. Apps appears when an app reports `installed`. This computer is a sidebar title with Disks and Repair under it, after Sign in. Settings appears then too. That gate is the sidebar, not the API. Notes at the bottom, then Power. Home greets you and shows Files, Away, and Disks; the disk total is capacity. The bar under a disk is the room left. Interaction rules in [ui.md](ui.md). Not a tab per subsystem. Color, space, type, and shape are names on `:root` in `web/style.css`. Eyebrows share one rule. Card titles (`h2.home-title`) are left out of it. `--pad-control` and `--pad-card` are the two paddings, both built from the space scale.
- `GET /api/shares` still returns the folder password with no session. That is the open hole. Loopback stays trusted so the Keel screen keeps working.
- Whole-disk install is ISO-only: the sidebar Install item and `POST /api/installer/plan` require a live USB (`KEEL_INSTALLER_ENABLED=1`, helper, and `/run/live/medium/live/filesystem.squashfs`). The installed app does not show Install. `/api/health` `can_install` is how the chrome decides. The helper writes `/run/keel/install.json`; the page polls `GET /api/installer/progress`. Tool output stays in `/run/keel/install.log`.

## Objects

- Disk
- Pool (plan persisted)
- Share (SMB on Linux; Windows copy cannot serve folders)
- Note (scratch pad)
- System snapshot (CPU, RAM, process RSS)
