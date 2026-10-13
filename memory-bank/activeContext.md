# Active context

## Focus

Bare-metal is now the primary production deployment track. The user should be
able to install Keel on a whole dedicated computer from Windows without an
existing target OS. USB is the first transport; PXE/iPXE and BMC virtual
media are planned transports. A VM remains the safe test environment.

GUI shell: **window is the frame**. Sidebar everyday rows are Home, Files, Apps, Away. Disks, Install, and Repair open from a **This computer** menu to the right so the column never scrolls. Guide, Settings, and Sign in sit above Notes. Notes stays at the bottom, then health and Shut down. Settings is where GitHub updates live. Rest is reached from Home.

rok catalog is **ship-with**. Idle = Tailscale floor (files + GUI from away). Active = demand, not all-on.

- `cargo run` serves GUI on port 9478
- Mock disks on non-Linux
- Pools/shares: pools persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel starts a house-network folder `files` (`\\ip\files`, name `keel`, password `keel` until real sign-in). Windows cargo run cannot serve that folder.
- After Proxmox sign-in, a session stays in RAM so Pools can list OpenCloud / storage content (Check files). Password is not written to disk.
- Pools list says Use this / Leave this. Machine names stay in the explanation by default; “Show the names that computer uses” puts them in the heading for people who already know them.

## What to do on this Windows copy

Click around. Write ideas in Notes. Sign in on Pools to list files on rok (OpenCloud), without wiping anything. Do not wipe rok. Real mounts wait for Linux.

The installable live image was rebuilt **2026-10-13**: `dist/baremetal/release/keel-amd64-install.iso`. SHA-256 `638725073388db612e2294728b611de7e3e2dd2b549244f8575e3a116d66b8c3`. It includes Settings Connect GitHub (device-flow code), token save, and live Install progress (one sentence and a bar; tool output stays in `/run/keel/install.log`). Install layout is a 1 MiB BIOS boot partition, a 512 MiB EFI partition, then the rest for Keel. After the copy, the installer drops the USB startup (stable admin token, no “disk not changed” banner), writes hosts, drops the USB apt source, rebuilds the initramfs, and installs both EFI and BIOS GRUB. Copy that ISO onto Ventoy and run the install again on the same disk. Do not write USB or touch rok unless asked. File Explorer on this house network can open `\\<keel-ip>\files` with name `keel` and password `keel`. On USB try-mode that folder is temporary. The GitHub token is not in the ISO; save it in Settings after the box is installed.

Signed program updates: Settings checks GitHub, verifies `SHA256SUMS.sig`, and can install the program on a Linux machine that has `/usr/lib/keel/rollback-update`. A failed start or a power cut before `/api/health` restores the previous program and settings. The private key is the GitHub secret `KEEL_RELEASE_SIGNING_KEY` only. The repository is private. Settings **Connect GitHub** shows a device-flow code; approve on a phone or this PC. The token is saved to `KEEL_DATA/github.token` (the page never shows it again). A pasted token and `KEEL_GITHUB_TOKEN` still work. The GitHub app id is public and may be saved on the box; it must not include a client secret. `install-keel` does not write that token, and it must not be baked into the ISO. Whole-disk A/B (RAUC) is still not built.

## Recent

- Sidebar now follows Cerebro’s sections: everyday rows, a flyout for Disks / Install / Repair, Guide/Settings/Sign in above Notes, `aside { overflow: hidden }`. Look stays sea-glass. No ISO remaster for this.

## Next

- Copy the new ISO onto Ventoy and reinstall on the Samsung disk so Settings can save the GitHub token and Install shows live progress.
- Boot the installer in a disposable VM before the next real disk
- Build `keel-amd64-repair.iso` on Linux/WSL: `sudo KEEL_REPAIR=1 ./packaging/baremetal/remaster-official.sh`. Boot that USB once to replace `/usr/bin/keel` and `/usr/lib/keel/web` without wiping `/var/lib/keel`. The installer is off on that image.
- Whole-disk A/B updates (RAUC) after the single-disk installer has been proven
- Mount a foreign NFS/SMB folder into the Keel share (files that still live on rok)
- Show overlay address next to each share once the overlay is enrolled
- Optional later: remember a Proxmox API token instead of retyping the password
- Replace the house-network `keel`/`keel` folder password with real sign-in
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
- Overlay is how files work away from home. Keel prefers Headscale and supports Tailscale clients. Same share. Never stop it to save watts. Brand can change; the job cannot.
- Power advisor: even while Active, score busy vs parked and explain each save
- Ship the rok LXC set (including currently stopped guests); not a plugin bazaar
- Storage may stay on rok/TrueNAS (foreign mount). keel replaces the rest, not the bits, until we have a dedicated NAS box.
- Admin chrome stays on screen; lists scroll. Same as AI Suite.
- Listing ZFS inside Proxmox/TrueNAS: sign in on Pools. Password stays in the GUI on this PC, never in git or chat. keel stores name + host only.
- Overlay provider boundary now exists at `GET /api/overlay`; Headscale is the default declared provider, with Tailscale client compatibility. Windows reports unavailable rather than simulating Linux state.
- State-changing API routes accept `Authorization: Bearer ...` or a one-day `keel_session` cookie when `KEEL_ADMIN_TOKEN` is configured. Non-loopback binds refuse to start without that token. From another computer, Sign in can ask the Keel screen; that screen shows Allow / Not now under the page title, in the page flow. Only a loopback browser can answer. Sign in uses the same sidebar as the other pages. The admin token stays in a closed section for when nobody is at the screen. Approval routes live in `auth.rs`.
- Authenticated `POST /api/overlay/enroll` now runs the local Tailscale-compatible client on Linux for Headscale or Tailscale enrollment, and saves only the provider URL. `/overlay.html` exposes the flow.
- Read-only `GET /api/services` reports the fixed Keel, overlay-client, and SMB systemd units; Windows reports unavailable rather than mocking service state.
- `keelctl bootstrap` is now the Windows-to-Linux automation path: it uploads the binary and service, writes `/etc/keel/keel.env` from stdin, creates `/var/lib/keel`, enables Keel, and returns the GUI URL.
