# Troubleshooting

## The appliance screen is black

`journalctl -u keel-kiosk` on the recovery console (`Ctrl+Alt+F3`) says which of
these it is.

- **"no KMS display device"** — the kiosk stood down on purpose and the text
  banner has the URL. Either the GPU has no driver in the image, or the machine
  was booted from the fail-safe entry, which sets `nomodeset`.
- **cage exits on "XDG_RUNTIME_DIR is not set"** — `pam_systemd` did not create
  the session directory. `start-kiosk.sh` falls back to `/run/keel-kiosk`; if it
  still fails, the PAM stack at `/etc/pam.d/keel-kiosk` is not being applied.
- **cage exits on seat or permission errors** — libseat could not take seat0
  through logind. The fallback used by the cage project is to install `seatd`,
  enable `seatd.service`, and add `keel-kiosk` to the `seat` group. We rely on
  logind instead, so reach for this only if logind genuinely will not work
  (cage-kiosk/cage#348).
- **The screen is blank but the box answers on the LAN** — keel is fine and only
  the kiosk is broken. Admin from Windows while you debug.

## GUI troubleshooting

Use Repair in the app: sentence + Fix. Do not start in Settings. AI Suite is not required.

## cargo run fails to bind 9478

Another process owns the port. Set `KEEL_PORT` or stop the other listener.

## GUI is not reachable from another machine

The development daemon binds to `127.0.0.1` by default. Set `KEEL_BIND` to the intended LAN address while testing. Do not expose it to a LAN or WAN until Keel's admin authentication is enabled.

## Empty or mock disks on Windows

Expected. Inventory is mock unless `cfg(target_os = "linux")`.

## GUI or SMB unreachable on the LAN

Check same L2/VLAN as the client, Proxmox NIC on the LAN bridge (not NAT), and firewalls for 9478 (GUI) and 445 (SMB). mDNS failing is not a daemon failure if `http://ip:9478` works. See intranet.md.

## lsblk missing on Linux

Install `util-linux`. keel falls back to an error disk entry rather than crashing.

## Sign-in notice does not appear on the Keel screen

The other computer uses **Ask the Keel computer** on Sign in. The notice with **Allow** is drawn by the page already open on the Keel screen, which must be `http://127.0.0.1:9478`. A browser that opened the LAN address cannot press Allow. The request expires after two minutes.

## Installer stops on wipefs `/dev/sda2` probing initialization failed

`wipefs` on the whole disk tells the kernel to drop the old partition nodes. Wiping `/dev/sda2` after that fails with `probing initialization failed: No such file or directory`, and the script stops before `mkfs`. The partition table on the selected disk is already cleared; Keel is not installed. Partition signatures have to be wiped first, while those nodes still exist, and the new `/dev/sda1` and `/dev/sda2` have to exist before the final wipe and format. The running USB still has the old helper until that image is rebuilt, or until `/usr/lib/keel/install-keel` on the live system is replaced. Run the install again on the same disk.

## Installer stops: `mkfs.vfat: command not found`

The live image formats the chosen disk itself. Debian's live system does not include `mkfs.vfat`, `mkfs.ext4`, `unsquashfs`, or GRUB. Those packages are installed by `install-disk-tools-in-rootfs.sh` on a full image build. A fast rebuild skips that step, so an image built before the script existed stops at `mkfs.vfat` after the partition table is already written. Keel is not installed. Use an image built after that script ran, and run the install again on the same disk.

## Installer stops: GRUB will not proceed with blocklists

EFI GRUB can finish, then BIOS GRUB stops because a GPT disk has no BIOS boot partition. The script then never reaches `update-grub`, so the disk is not a finished Keel. The layout is a 1 MiB BIOS boot partition, a 512 MiB EFI partition, then the rest for Keel. Run the install again on the same disk with an image that contains that layout.

## Install page filled with wipefs or unsquashfs text and would not scroll

The window does not grow a page scrollbar. Older images waited until the helper finished, then pasted every tool line into one paragraph. The current image starts the install in the background and the page shows one sentence plus a bar. Tool output is in `/run/keel/install.log` on the USB session. Use an image built after that change.

## Installed disk still behaves like the USB

Copying the live system onto the disk is not the whole install. Left alone, the startup script mints a new admin token on every boot (the token printed at the end of install would not match), the screen still says the disk was not changed, `/etc/hosts` is empty, and apt still looks for the USB. `live-config` itself does not run unless the kernel command line says `boot=live`. The installer now switches the copy over: stable token, loopback sign-in kept, USB banner removed, hosts and apt fixed, and a new initramfs built for the installed disk.
