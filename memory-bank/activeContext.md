# Active context

## Focus

Bare-metal is now the primary production deployment track. The user should be
able to install Keel on a whole dedicated computer from Windows without an
existing target OS. USB is the first transport; PXE/iPXE and BMC virtual
media are planned transports. A VM remains the safe test environment.

GUI shell: **window is the frame**. Sidebar everyday rows are Home, Files, and Away. Apps stays hidden until one is installed. Disks and Repair open from a **This computer** menu after Sign in. Install is only on the live USB image, not in the installed app. Guide stays above Notes. Settings appears after Sign in; Sign in hides once `/api/auth/session` says signed in (the Keel screen always is). Notes stays at the bottom, then health and a **Power** button (icon). Signed in, its menu is Sign out (browser sessions only), Restart, Shut down. Until then the menu is Sign in. Home greets you, names one thing to look at, and shows Files, Away, and Disks. Fix it opens Sign in, then Disks. Rest is a Home link.

Apps today: the Apps page lists the catalog, but the image ships no app programs and no `keel-app@.service`, so Start does nothing yet. Status there is computed from Rest/Active, not read from systemd. The Tailscale client is also not in the image, so Away cannot enroll from Keel yet. Samba (`\\ip\files`) is the only file path Keel itself serves. Sessions live in RAM, so a Keel restart or update signs browsers out.

rok catalog is **ship-with**. Idle = Tailscale floor (files + GUI from away). Active = demand, not all-on.

- `cargo run` serves GUI on port 9478
- Mock disks on non-Linux
- Pools/shares: pools persist in `data/pools.json`. Shares persist in `data/shares.json`. Linux Keel starts a house-network folder `files` (`\\ip\files`, name `keel`, password `keel` until real sign-in). Windows cargo run cannot serve that folder.
- After Proxmox sign-in, a session stays in RAM so Pools can list OpenCloud / storage content (Check files). Password is not written to disk.
- Pools list says Use this / Leave this. Machine names stay in the explanation by default; “Show the names that computer uses” puts them in the heading for people who already know them.

## What to do on this Windows copy

Click around. Write ideas in Notes. Sign in on Pools to list files on rok (OpenCloud), without wiping anything. Do not wipe rok. Real mounts wait for Linux.

The installed box, as of 2026-10-15, answers at `http://192.168.0.191:9478/` and is Linux Keel **0.1.5**. Settings offered signed **v0.1.7** and rolled it back: “Keel 0.1.7 did not answer.” The 0.1.5 installer saves the new program without permission to run, so another Settings try of that release ends the same way. Notes there are empty, and saving one is refused until Sign in. The family Home and the design-piece cleanup ship in signed v0.1.8. The signed v0.1.7 package does not contain that Home page; it was tagged before the page was built.

The installable live image was rebuilt **2026-10-13**: `dist/baremetal/release/keel-amd64-install.iso`. SHA-256 `638725073388db612e2294728b611de7e3e2dd2b549244f8575e3a116d66b8c3`. It includes Settings Connect GitHub (device-flow code), token save, and live Install progress (one sentence and a bar; tool output stays in `/run/keel/install.log`). Install layout is a 1 MiB BIOS boot partition, a 512 MiB EFI partition, then the rest for Keel. After the copy, the installer drops the USB startup (stable admin token, no “disk not changed” banner), writes hosts, drops the USB apt source, rebuilds the initramfs, and installs both EFI and BIOS GRUB. Copy that ISO onto Ventoy and run the install again on the same disk. Do not write USB or touch rok unless asked. File Explorer on this house network can open `\\<keel-ip>\files` with name `keel` and password `keel`. On USB try-mode that folder is temporary. The GitHub token is not in the ISO; save it in Settings after the box is installed.

The repair image was rebuilt **2026-10-15** at 07:40: `dist/baremetal/release/keel-amd64-repair.iso` (2,246,311,936 bytes). SHA-256 `c0102c3fdaded192a3716ba2e47748112007d15d0b179fbfd1a391ea557f0814`. It is a fast rebuild of this working copy: Linux Keel 0.1.7, payload mode `0755`, and the current `web/` including the family Home. It replaces `/usr/bin/keel` and `/usr/lib/keel/web` on `LABEL=KEEL-ROOT` and leaves `/var/lib/keel` alone. The disk installer is off. The 2026-10-13 file (SHA-256 `06959ff94a0ce489d6aed83d802f0bbab7262c9905809287887503da6537ac0b`) is no longer at that path. Copy this ISO onto Ventoy. It has not been written to a USB. Boot it once, wait until the screen says to remove the USB, then restart from the Samsung disk.

Signed program updates: Settings checks GitHub, verifies `SHA256SUMS.sig`, and can install the program on a Linux machine that has `/usr/lib/keel/rollback-update`. A failed start or a power cut before `/api/health` restores the previous program and settings. The private key is the GitHub secret `KEEL_RELEASE_SIGNING_KEY` only. The repository is private. Settings **Connect GitHub** shows a device-flow code; approve on a phone or this PC. The token is saved to `KEEL_DATA/github.token` (the page never shows it again). A pasted token and `KEEL_GITHUB_TOKEN` still work. The GitHub app id is public and may be saved on the box; it must not include a client secret. `install-keel` does not write that token, and it must not be baked into the ISO. Whole-disk A/B (RAUC) is still not built.

## Recent

- **Repair ISO (2026-10-15):** `dist/baremetal/release/keel-amd64-repair.iso`, SHA-256 `c0102c3fdaded192a3716ba2e47748112007d15d0b179fbfd1a391ea557f0814`. The payload program is mode `0755`. The family Home is in the copied pages. No physical disk was written.
- **v0.1.7 Settings install rolled back (2026-10-15):** the box stayed on 0.1.5. The signed release is on GitHub. The hop off 0.1.5 is the repair USB above, not another Settings click.
- **Design levers (2026-10-15):** eyebrows share one size and tracking. Card titles are `h2.home-title` and are left out of that rule. Buttons and sea-glass links share `--pad-control`. Menus share one surface. `--pad-control` and `--pad-card` are built from the space scale.
- **Family Home (2026-10-15):** Home greets you and shows Files, Away, and Disks. The big disk figure is how much the disks hold, not free space. The notice is sea-glass. Fix it opens Sign in, then Disks, and does not erase. A person’s name is not written into the page. The sidebar hides Apps until an app reports `installed`, and hides Disks, Repair, Settings, Restart, and Shut down until Sign in. That hide is the page chrome. The JSON endpoints are still open, and `GET /api/shares` still returns the folder password. On this Windows copy there is no admin token, so the session counts as signed in and the admin rows stay visible.
- **Design pieces (2026-10-15):** color, space, type, and shape are names in `web/style.css` `:root`. Cards, tables, notices, buttons, and fields share those names. A page does not invent its own size or color. Sea-glass stays. Power stays the sidebar button.
- **Saved computers (2026-10-15):** Disks keeps each computer you look at, and what answered, in `remote.json`. Opening the page shows that again. Look again checks once more. The password is not kept.
- **Files page (2026-10-15):** open a folder, see what is in it, make a folder, move or copy. Pictures stay files. On Linux this only opens a mounted Keel disk under `/srv/keel`, and Copies stays read-only. On this Windows copy the page uses sample folders in `data/mock-files`. New files on the Keel computer are given to the share's name.
- **Disks step 1 built (2026-10-14, ships in v0.1.7):** Disks page left column "In this computer" lists every disk with what it holds (Keel itself / a job / In use / Empty / Old data) and **Use…** → pick a job → type the disk name → Erase and use. Jobs: Files (Documents, Work, Media), Nightly copy (Copies folder, read-only, `Latest/` + `Older/<date>/` kept 30 days), Third copy (Documents and Work), Software and Inbox. Each disk is one ext4 partition labelled `keel-<job>`, mounted at `/srv/keel/<job>` by UUID with `nofail,x-systemd.device-timeout=10s`. Host changes (wipefs, parted, mkfs, `/etc/fstab`, mount, useradd, smbpasswd) run via `systemd-run --wait --pipe` because keel.service has `ProtectSystem=full` and a private mount view (`src/storage.rs`). State in `KEEL_DATA/storage.json`. Shares on a disconnected disk are hidden from Samba (never write into an empty mount dir). Daily copy is Keel's own Rust mirror (`src/copies.rs`, no rsync), checked each minute, Copy now button, Repair shows red for a missing disk and yellow for failed or 2-day-old copies. Files page "Who opens these folders": one name + password (Samba user, nologin) closes guest access; keel/keel stays only until a name is set. Not yet tested on real hardware.
- Sidebar now follows Cerebro’s sections: everyday rows, a flyout for Disks / Repair, Guide/Settings/Sign in above Notes, `aside { overflow: hidden }`. Install is only on the live USB image (`/api/health` `can_install`). Look stays sea-glass.
- Remaster checksum hashes the basename in use (`keel-amd64-repair.iso` in repair mode), not always the install ISO name.
- Boot errors on the installed disk are already in `/var/log/journal`. `quiet` still prints them on the black screen. The release image has no SSH, so they are not easy to copy until Repair shows them. Bluetooth and `wpa_supplicant` start because Debian live included them. Keel has no Wi-Fi or Bluetooth page.

## Next

- Copy `dist/baremetal/release/keel-amd64-repair.iso` (2026-10-15) onto Ventoy and boot it once on the Keel computer. After restart from the Samsung disk, Home should report a version newer than 0.1.5 and show the family page. Then Disks → WD 500 GB Files, Seagate 500 GB Nightly copy, Netac 720 GB Third copy, Toshiba 500 GB Software and Inbox; leave the NVMe for app databases. Then Files → one folder name and password.
- Faster startup, after that hop: stop printing, Bluetooth, `wpa_supplicant`, Avahi, cron, and sensors from starting. Start Keel when the disk is mounted, not after the network and Samba. The screen still waits for `/api/health`. The repair USB does not do this. Time a boot before changing the kiosk browser.
- Code, after that hop is possible: take `password` out of `GET /api/shares`. Require a session for admin reads (full disk paths, power, pools, saved computers, updates, repair actions) when the request is not the Keel screen. Leave health, hostname, addresses, share paths, Away state, and disk names with sizes open so Home still loads. Escape Apps, Away, Repair, Install, Settings, and Rest. A typed `/pools.html` or `/repair.html` should say Sign in when the data is refused.
- Later, once the password is gone and the folders sit on the data disks: SMART and free space (a short bar means filling up; red means failing), firewall for the house network and Away only, a phone alert when something is red, HTTPS, then one folder login per person.
- Check `/etc/apt/sources.list` on the installed box has `deb.debian.org` lines; if yes, Podman and Tailscale can be installed online from Keel (no USB). Container images always download from the internet.
- Plug the Yottamaster into the Keel box in JBOD/Normal (not the enclosure’s RAID). NVMe is OS/apps; USB bays are bulk files. Do not put ZFS on the USB bridge.
- Overlay stays the away door for the same share. Do not expose SMB to the public internet.
- Do not wipe rok. OpenCloud on rok can stay until files are copied or mounted as a foreign folder.
- Copy the install ISO onto Ventoy and reinstall on the Samsung disk only when a full disk install is wanted.
- Whole-disk A/B updates (RAUC) after the single-disk installer has been proven
- Mount a foreign NFS/SMB folder into the Keel share (files that still live on rok)
- Show overlay address next to each share once the overlay is enrolled
- Optional later: remember a Proxmox API token instead of retyping the password
- Replace the house-network `keel`/`keel` folder password with real sign-in
- Wire idle sensors for the advisor (disk/CPU)
- Package the rok catalog (including 106, 109)

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
- **Apps (2026-10-14):** use existing apps, not Keel rewrites. Run them as Podman containers, each a systemd unit (Quadlet); no Docker daemon. Download only when chosen (first-start “What will you use Keel for?” ticks, or Apps → Install). Versions are pinned and signed with the Keel release; Keel sets up folder, NVMe database, ports, Away address, first account, and shows a phone QR. Containers see only the folders Keel grants. Remove keeps data. App updates ride Keel releases with rollback.
- **Folders (shares built in step 1; subfolders are the user's to make):** five shares — Documents (private: Bills and Taxes, Financial and Banking, Identity, Legal and Residency, Education and Professional History, Requests and applications, Travel), Work (Business, NGOs, Projects, Presentation, Design and communication, AI Knowledge), Media (Photos, Videos, Music), Software (Apps, Games, Office; no backup), Inbox (was Temp; no backup). Optional Family share if others use Keel. Photos are files in Media. Immich, later, only shows those picture files; it does not hold them.
- **Order (2026-10-15):** file management first. Disks and the shared folders, then a way to see and move the files, then Tailscale so the same folders work away from home. Catalog apps wait. Immich is not the first app.
- **Scanopy (2026-10-15):** not a Keel app. Disks already keeps the computers you looked at and what answered. A map, if one is needed later, is Keel's own page.
- **Proxmox Backup Server (2026-10-15):** not a Keel app. Nightly copy and Third copy already keep the files. It stays off the Apps list.
- **OpenCloud (2026-10-15):** removed from the Apps list. Files already lists, makes, moves, and copies on the Keel disks, and a PC opens the same folders. The copy still running on rok can be listed from Disks until those files are copied. A phone that cannot open a shared folder can later put a file in and take one out from Files, on those same folders.
- Storage may stay on rok/TrueNAS (foreign mount). keel replaces the rest, not the bits, until we have a dedicated NAS box. The Yottamaster on the Keel computer is that data box: NVMe is OS and apps; USB bays are bulk files. Enclosure mode is JBOD/Normal so Linux sees each disk; protection is SnapRAID or a second copy, not the enclosure’s RAID and not ZFS over USB. Snapshots and RAID are not a backup; a second copy still has to live somewhere else.
- Admin chrome stays on screen; lists scroll. Same as AI Suite.
- Listing ZFS inside Proxmox/TrueNAS: sign in on Pools. Password stays in the GUI on this PC, never in git or chat. keel stores name + host only.
- Overlay provider boundary now exists at `GET /api/overlay`; Headscale is the default declared provider, with Tailscale client compatibility. Windows reports unavailable rather than simulating Linux state.
- State-changing API routes accept `Authorization: Bearer ...` or a one-day `keel_session` cookie when `KEEL_ADMIN_TOKEN` is configured. Non-loopback binds refuse to start without that token. From another computer, Sign in can ask the Keel screen; that screen shows Allow / Not now under the page title, in the page flow. Only a loopback browser can answer. Sign in uses the same sidebar as the other pages. The admin token stays in a closed section for when nobody is at the screen. Approval routes live in `auth.rs`.
- Authenticated `POST /api/overlay/enroll` now runs the local Tailscale-compatible client on Linux for Headscale or Tailscale enrollment, and saves only the provider URL. `/overlay.html` exposes the flow.
- Read-only `GET /api/services` reports the fixed Keel, overlay-client, and SMB systemd units; Windows reports unavailable rather than mocking service state.
- `keelctl bootstrap` is now the Windows-to-Linux automation path: it uploads the binary and service, writes `/etc/keel/keel.env` from stdin, creates `/var/lib/keel`, enables Keel, and returns the GUI URL.
