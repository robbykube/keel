# Active context

## Focus

Bare-metal is now the primary production deployment track. The user should be
able to install Keel on a whole dedicated computer from Windows without an
existing target OS. USB is the first transport; PXE/iPXE and BMC virtual
media are planned transports. A VM remains the safe test environment.

GUI shell: **window is the frame**, with Cerebro’s interaction rules (`memory-bank/ui.md`): sidebar never scrolls, Notes at the bottom, form beside list, Save-button rule, not-running is grey. Look stays sea-glass, not Cerebro yellow. Wide pages use two panes so they fill the window (Guide words | rest; Pools look/sign-in | add/list).

rok catalog is **ship-with**. Idle = Tailscale floor (files + GUI from away). Active = demand, not all-on.

- `cargo run` serves GUI on port 9478
- Mock disks on non-Linux
- Pools/shares: pools persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel starts a house-network folder `files` (`\\ip\files`, name `keel`, password `keel` until real sign-in). Windows cargo run cannot serve that folder.
- After Proxmox sign-in, a session stays in RAM so Pools can list OpenCloud / storage content (Check files). Password is not written to disk.
- Pools list says Use this / Leave this. Machine names stay in the explanation by default; “Show the names that computer uses” puts them in the heading for people who already know them.

## What to do on this Windows copy

Click around. Write ideas in Notes. Sign in on Pools to list files on rok (OpenCloud), without wiping anything. Do not wipe rok. Real mounts wait for Linux.

The live USB image was remastered **2026-09-14 20:39** (`dist/baremetal/release/keel-amd64.iso`, Debian live 13.7.0 plus Samba). That file is older than the guarded installer and the signed-update units. File Explorer on this house network can open `\\<keel-ip>\files` with name `keel` and password `keel`. On USB try-mode that folder is temporary. Copy a newly built ISO onto Ventoy’s data partition. Do not write USB or touch rok unless asked.

Signed program updates: dashboard checks GitHub, verifies `SHA256SUMS.sig`, and can install on a Linux machine that has `/usr/lib/keel/rollback-update`. A failed start or a power cut before `/api/health` restores the previous program and settings. The private key is the GitHub secret `KEEL_RELEASE_SIGNING_KEY` only. The repository is private, so the appliance also needs `KEEL_GITHUB_TOKEN` in its environment before it can see a release. Whole-disk A/B (RAUC) is still not built.

## Next

- Build a fresh ISO and boot the installer in a disposable VM before a real disk
- Publish the next signed tag (`v0.1.2`) when there is a program change to ship
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
- State-changing API routes accept `Authorization: Bearer ...` when `KEEL_ADMIN_TOKEN` is configured. Non-loopback binds refuse to start without that token; browser login/session remains next.
- Authenticated `POST /api/overlay/enroll` now runs the local Tailscale-compatible client on Linux for Headscale or Tailscale enrollment, and saves only the provider URL. `/overlay.html` exposes the flow.
- Read-only `GET /api/services` reports the fixed Keel, overlay-client, and SMB systemd units; Windows reports unavailable rather than mocking service state.
- `keelctl bootstrap` is now the Windows-to-Linux automation path: it uploads the binary and service, writes `/etc/keel/keel.env` from stdin, creates `/var/lib/keel`, enables Keel, and returns the GUI URL.
