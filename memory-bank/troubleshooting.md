# Troubleshooting

## The computer only starts while the USB is in

Reported 2026-10-15 evening. The repair USB got far enough that the update looked finished, Settings could not install the GitHub update, and with the USB removed the machine stays on its loading picture. This PC could not reach `192.168.0.191:9478`. The repair copy does not change the boot program. A GitHub update is refused while `/run/live/medium` exists, so it cannot be applied from the USB session. Start from the Samsung disk using the firmware boot menu. If that disk's own page never appears, boot the repair USB again. It does not wipe the disk.

## The screen stays on the loading picture after the USB is chosen

The repair USB hides boot text on purpose. From the menu until the page appears, the screen is blank or still shows the computer’s own loading picture, and the cursor is hidden. Reading the system off the USB takes a few minutes. This PC could not reach `192.168.0.191:9478` during that wait (15 Oct 2026). If the page has not appeared after about five minutes, hold the power button, remove the USB, and start from the Samsung disk. The disk is unchanged unless the screen already said to remove the USB.

## Settings says the download failed while decoding the response

The check can succeed and still stop on Install. 0.1.8 allows 20 seconds for the whole file, then reports a stopped read as “download failed: error decoding response body.” The signature files are small, so they pass, and the package is the one that stops. 0.1.11 allows the file to finish and names the reason underneath that sentence. A retry from 0.1.8 can still hit the same 20 second stop.

## Red lines on the black screen while Keel starts

The installed disk boots with `quiet` only (`/etc/default/grub`). That still prints kernel errors, then the page covers them. The USB image adds `loglevel=3`, so the same lines stay off the USB screen. Linux already keeps them: `/var/log/journal` is on the disk, so they survive a restart. The release image has no SSH, and the recovery console is not a sign-in set up for copying that log. A photo is the capture until Repair can show the lines and a Copy button.

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

## The installed box cannot take a Settings update

A USB **repair** image updates only `/usr/bin/keel` and `/usr/lib/keel/web`. It does not run the disk installer. Build it with `sudo KEEL_REPAIR=1 ./packaging/baremetal/remaster-official.sh`. The file `dist/baremetal/release/keel-amd64-repair.iso` built 2026-10-15 (SHA-256 `c0102c3fdaded192a3716ba2e47748112007d15d0b179fbfd1a391ea557f0814`) is this working copy: Keel 0.1.7 with permission to run, plus the current pages. Copy it to Ventoy, boot it once, wait until the screen says to remove it, then restart from the disk. A leftover copy of the 2026-10-13 image (SHA-256 `06959ff94a0ce489d6aed83d802f0bbab7262c9905809287887503da6537ac0b`) would put 0.1.5 back.

## Settings says the new Keel did not start, so the previous version was restored

0.1.5 unpacked the new program as a normal file, without permission to run. systemd still reports that the restart worked. Nothing answers `/api/health`, so the restore script puts the previous program back. 0.1.6 and 0.1.7 from Settings hit that wall.

0.1.7 does mark the new program runnable. The 2026-10-15 USB put that 0.1.7 on the disk. Settings then tried 0.1.8 and still said “The new Keel did not start, so the previous version was restored. Keel 0.1.8 did not answer.” The restore service is ordered before Keel and starts again on that restart, because the unfinished-update marker is already written. It puts 0.1.7 back before 0.1.8 can answer. The restore script now leaves the new program in place once the computer is up. A power cut still restores, because the next start is a boot, not a restart. The repair USB copies that script and moves the service off Keel’s restart.

## Settings says a release is available but the signature is not valid

Keel 0.1.1 and 0.1.2 asked GitHub for the signed files with a JSON header. GitHub then sent asset metadata instead of `SHA256SUMS.sig`, and Settings treated that as a bad signature. The v0.1.2 files themselves verify. 0.1.3 downloads the files as raw bytes. A 0.1.1 or 0.1.2 box cannot install that fix from Settings; copy `v0.1.3` onto the machine once, then later updates work.
