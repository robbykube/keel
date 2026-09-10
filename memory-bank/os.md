# Appliance OS shape

keel is a **Debian-class appliance**, not a general Linux distro. We take the kernel, glibc, systemd, and a named package list from Debian stable. We do not take “Debian” as a lifestyle (no desktop, no tasksel, no recommended-by-default pile).

`keel` is the control plane. systemd is PID 1. No graphical login on the box.

## Effective / quick / accurate

| Word | Means for keel |
| --- | --- |
| Effective | Disks → pool → share; catalog on demand; overlay floor so files work away; Guide that tells the truth |
| Quick | Fast install (copy a baked root), fast boot (few units), fast GUI, idle watts |
| Accurate | Live unit state, real `lsblk`/SMART, checksummed image, clock sync *after* boot, advisor evidence not guesses |

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

- **Disk layer:** local ZFS/mdadm **or** a foreign NFS/SMB/iSCSI from TrueNAS/Proxmox. Not both pretending to be one vdev.

**Catalog:** separate image/stream. Installed, stopped. Not in the one-minute root.

**Do not take:** GNOME/KDE, snap, cloud-init, cups, avahi-by-default, popularity-contest, unattended-upgrades that block boot, `Recommends` flood (`apt --no-install-recommends`), extra syslog stacks, Plymouth themes, a local display manager.

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
