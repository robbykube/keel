# Appliance OS shape

keel is a **Debian-class appliance**, not a general Linux distro. We take the kernel, glibc, systemd, and a named package list from Debian stable. We do not take “Debian” as a lifestyle (no desktop, no tasksel, no recommended-by-default pile).

`keel` is the control plane. systemd is PID 1.

**The box has a screen and it is graphical.** keel is a newbie appliance: a
person who has never opened a terminal must be able to plug in a monitor and use
it. No login prompt, no scrolling boot text, no shell — ever, on the normal
path. The local screen is a Wayland kiosk (`cage`) running one full-screen
browser against `127.0.0.1:9478`, which is the same UI a Windows browser reaches
over the LAN. One codebase, two surfaces.

This reverses an earlier "no graphical login on the box" rule. That rule bought
a small root and low idle watts, and it cost the product its audience: the
appliance's own screen showed a Debian login prompt. The trade is now explicit —
chromium, mesa's DRI drivers and xwayland are roughly 500-700 MB installed
(rather less after squashfs compression), and the browser idles at a few hundred
MB of RAM. Idle mitigation is blank-and-stop, not removal.

A terminal still exists for recovery, on `Ctrl+Alt+F3` and over SSH. It is not
advertised and nothing on the normal path leads to it.

The initial service contract is in `packaging/systemd/keel.service`. The unit
is intentionally not the final privilege boundary: storage and network
mutations should move behind a narrow helper before production LAN exposure.

## Console trust

The kiosk cannot authenticate the way a LAN browser does. The admin token is
readable only from a terminal, and the terminal is exactly what the appliance
must never show. So `KEEL_TRUST_LOOPBACK=1` (set by the live drop-in, off
everywhere else) lets requests arriving on loopback skip the token. LAN requests
are unaffected and still need it.

The justification is physical: whoever is at the keyboard can already power-cycle
the box and boot it single-user, so they are root-equivalent regardless.

**Never put a reverse proxy in front of keel while this is on.** A proxy on the
same host makes every forwarded request look like loopback, which converts the
console exemption into an unauthenticated admin API for the whole network. keel
ignores `X-Forwarded-For` on purpose; trusting it would be the same hole with
extra steps.

The cost is that *every* local process gets admin, not just the kiosk — which
matters because catalog apps are meant to run natively on this host. The fix,
when catalog apps land: give keel a second listener on a unix socket owned
`root:keel-kiosk` mode 0660 and exempt that socket instead of an IP. Filesystem
permissions then name the kiosk exactly. Do this before the first native catalog
app ships.

## Effective / quick / accurate

| Word | Means for keel |
| --- | --- |
| Effective | Disks → pool → share; catalog on demand; overlay floor so files work away; Guide that tells the truth |
| Quick | Fast install (copy a baked root), fast boot (few units), fast GUI, idle watts |

**Startup (2026-10-15):** the page is late because the Debian live base still starts printing, Bluetooth, `wpa_supplicant`, Avahi, cron, and sensors, and `keel.service` waits for `network-online.target` and Samba before the program runs. The screen then waits for `/api/health` and starts Chromium from an empty profile under `/run`. Stop the extras. Start Keel when the disk is mounted, and let Samba and the network come up beside it. The repair USB replaces the program and the pages only, so it does not change this. The smaller kiosk browser in `size.md` is the remaining cut, and it shortens install as well.
| Accurate | Live unit state, real `lsblk`, checksummed image, clock sync *after* boot, advisor evidence not guesses |

**Firewall.** Repair’s “Only the house and Away” lets the page (9478) and the folders (445) answer on the house /24 and on the Tailscale interface. Loopback stays open for the Keel screen. Tailscale’s own port stays open so a wrong house rule does not cut off the phone. Other incoming connections are refused. Outbound stays open. The choice is remembered and applied again when Keel starts. It does not turn on by itself.

**Idle disks.** A button on Disks asks one spinning disk in the USB box to sleep. The check does not wake a disk that is already asleep. If it is already asleep, Keel leaves the box to do that and does not set its own timer. If the box ignores the request, the disks stay spinning.

**SMART** is read with `smartctl`. A sleeping disk is left asleep. A missing answer stays unknown, and that is not called a failure. A disk in a USB box still shows the room left. A SMART reading from that box is not shown as failing, because the box often answers for itself. `smartmontools` is named in `keel-packages.list` but the remaster does not use that list, so an installed computer may not have it yet. The first time disk health is read, Keel installs that package if it is missing. The big Home number stays capacity. The bar is the room left.

Speed comes from **less software running**, not from lying. Accuracy comes from **Debian’s kernel + our sensors**, not from reimplementing Linux.

## Take from Debian (named, or it does not ship)

**Always (the OS):**

- linux-image + microcode + firmware for NIC/storage (per machine delta)
- systemd, udev, util-linux (`lsblk`), coreutils, iproute2, ca-certificates
- openssh-server
- tailscale (or our unit wrapping it) — idle floor; path to files when not at home
- samba (or equivalent) when we wire shares — must listen on LAN and on the overlay
- smartmontools
- keel.service
- cage + chromium — the appliance screen; one full-screen browser on
  `127.0.0.1:9478`, no desktop, no window chrome, nothing to exit into.
  Debian's cage pulls in xwayland as a hard dependency. This is the largest
  thing in the image and roughly two thirds of it is avoidable — see
  `size.md` for the cog/WPE swap that drops the compositor entirely
- libgl1-mesa-dri + fonts-dejavu-core — what cage and the browser need to draw
  on real hardware
- curl — the kiosk waits on `/api/health` before drawing its first frame

**Not Plymouth, yet.** A splash draws from the initramfs, and the remaster reuses
Debian's stock `/live/initrd.img` instead of regenerating it, so Plymouth in the
squashfs would never run. Boot is silenced with `quiet loglevel=3
systemd.show_status=false` instead: blank rather than splashed. Real Plymouth
needs `update-initramfs` in the rootfs and the result mapped over
`/live/initrd.img`, which can produce an unbootable ISO and deserves its own
change.

- **Disk layer:** local ZFS/mdadm **or** a foreign NFS/SMB/iSCSI from TrueNAS/Proxmox. Not both pretending to be one vdev.

**Catalog:** separate image/stream. Installed, stopped. Not in the one-minute root.

**Do not take:** GNOME/KDE, snap, cloud-init, cups, avahi-by-default, popularity-contest, unattended-upgrades that block boot, `Recommends` flood (`apt --no-install-recommends`), extra syslog stacks, a local display manager. Wi-Fi and Bluetooth are not Keel features. The Debian live base still starts `bluetooth.service` and `wpa_supplicant.service`, and it includes some Wi-Fi firmware (Intel files are in the image). There is no NetworkManager and no page to join a network or pair a device. The computer is meant to use a cable. Printing (`cups`) and `avahi-daemon` also start because they came with that base; they are not chosen Keel services.

A kiosk compositor is not a desktop. `cage` draws one window and has no
launcher, no file manager, no settings app, and no way for a user to reach a
second program. A display manager is still refused: there is no one to log in
as, because there is no login.

Build with `debootstrap --variant=minbase` (or equivalent) plus the list above. If a package has no sentence in this file, it does not go in the root image.

## Native packages, not a container host

This OS is **not Proxmox**. Floor and most catalog jobs are **.debs + systemd units** on the host. Containers do not save idle CPU; a **stopped process uses ~0**. A running container costs *more* (shim, extra mounts), not less.

Idle = `systemctl stop` (or never start) everything except `keel` and `tailscale`. That is the core/CPU save. Same idea as a desktop: apps are installed; you open one when you need it; the rest stay closed. The overlay stays up so a laptop away from home still reaches the same share. Samba is either on that floor or starts when the share is hit — opening a file from away must not require hunting the catalog.

When a unit *is* running, save further with systemd (CPUQuota, IOWeight), the power advisor (park quiet apps), and HDD spindown — still no hypervisor.

**Incus/podman only** if an app’s deps fight the host (typical: Immich, Jitsi). Even then idle is stop/park the container. Do not wrap Samba or Tailscale “for efficiency.”

1. Minimal Debian root (baked, copied at install)
2. `keel.service` — always on
3. Catalog as **systemd units** (native). Incus only when deps require it — not to save watts.
4. ISO: root stream + optional catalog stream

## Idle

`keel` + `tailscale` running. Other catalog units stopped. File shares still reachable on the overlay (Samba on the floor or warm-started).

## Install speed

Copy Omarchy’s mechanics: offline baked tree, `btrfs send|receive` (or unsquashfs) not live debootstrap, prefetch ISO while the wizard runs, skip NTP/mirrors during install, checksum the image. Split floor vs catalog so Immich does not block first GUI.

## This repo

Control plane first. Catalog in the GUI is the spec for units; it does not start real guests on Windows.
