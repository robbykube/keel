# Keel bare-metal image

Keel's production target is a whole dedicated computer. A VM is only the
safe development path. This directory defines the first bootable appliance
image; it is intentionally separate from `keelctl`, which assumes Linux and
SSH already exist.

## Windows-to-bare-metal installation

### USB writing choice

Keel should support both common Windows tools rather than forcing users to
learn a Linux command.

- **Rufus is the default recommendation for the first installer release.**
  It writes one verified Keel ISO to a selected USB drive and has a simple,
  familiar flow. The Keel GUI can guide the user to download Rufus from its
  official site, verify the Keel checksum first, and then select the USB by
  name and size. Rufus supports creating bootable USB media from ISO images.
  [Rufus](https://rufus.ie/)
- **Ventoy is excellent for development and hardware testing.** Install
  Ventoy once, then copy several Keel ISO versions to the USB and choose one
  from the boot menu. This makes it easy to compare a stable build, a test
  build, and a recovery build. Ventoy's own documentation describes this
  copy-and-boot workflow. [Ventoy](https://www.ventoy.net/en/)

Keel's future Windows installer should not require either tool for normal
users; it should provide its own guided “Create Keel USB” button. Under the
hood, that button may use a trusted raw-image writer. Rufus and Ventoy remain
useful fallback tools and should be documented clearly.

The supported paths are:

1. **USB image (first release, recommended).** Windows downloads or builds a
   Keel image, verifies its SHA-256 checksum, and writes it to a USB drive.
   The target boots from that USB and shows the Keel installer GUI. The target
   does not need an existing OS or internet access. Network access is only
   needed for optional online steps such as Headscale enrollment or fetching
   packages not included in the image.
2. **PXE/iPXE (next).** The target and Windows machine are connected to the
   same isolated LAN. The target firmware must support network boot. Windows
   hosts the boot files and installer payload; the switch does not need an
   upstream internet connection. DHCP must be planned carefully so it does
   not conflict with the normal LAN DHCP server.
3. **BMC/IPMI/Redfish virtual media (hardware-dependent).** A server with a
   management controller can boot an ISO uploaded from Windows, without an OS
   on the target. This is useful for remote or headless hardware.

Windows cannot SSH into a completely blank machine: SSH is a post-boot path.
The future Windows GUI should make the USB path one guided flow and expose
PXE/BMC only when the user chooses those advanced methods.

## The easiest test: USB on a spare computer

The first test should not touch the Proxmox installation.

1. On Windows, open Keel's installer tool.
2. Download or select the Keel image.
3. Insert a spare USB drive. The tool shows the drive name, size, and a clear
   warning that writing the image will erase that USB drive.
4. Confirm the drive and let the tool verify the image checksum, write the
   image, and safely eject the USB.
5. Shut down a spare computer and boot it from the USB. Select the USB in the
   computer's one-time boot menu, usually opened with F8, F11, F12, or Esc.
6. Keel starts its installer and displays the setup screen on the spare
   computer. The user selects the target disk, confirms the disk name and
   size, and only then begins installation. (Intended product. The current
   image boots into the GUI but its installer page cannot yet write a disk —
   see "Build a Ventoy test ISO" below.)
7. After the first reboot, connect the Keel computer to the same local
   network as Windows. Windows opens the Keel address shown by the installer
   and finishes setup in the browser.

The normal path should require no PowerShell, Command Prompt, SSH, or Linux
terminal. A local console is retained as a recovery option, not as part of
normal setup. A detailed in-product guide will explain every screen in plain
language and include “What will happen?”, “Why is this needed?”, and “How do
I undo this?” help.

## Can Windows run Keel from the USB?

There are three different meanings of “run from USB”:

- **Boot the spare computer from USB:** yes, and this is the recommended first
  test. Windows remains installed on the spare computer; Keel runs only while
  that computer is booted from the USB. Windows on your main PC can manage it
  through a browser.
- **Install Keel onto the spare computer's internal disk:** yes. This is the
  realistic appliance test, and it still leaves Proxmox on your other machine
  completely untouched.
- **Run the Keel USB directly inside the current Windows session:** not as a
  normal Windows application. Windows cannot use the same USB as a native
  booted Linux appliance while it is running. We can instead run the image in
  a Proxmox/Hyper-V/VMware VM, or reboot the Windows PC and boot the USB.

For early development, the safest progression is:

| Test | Where Keel runs | Does it affect Proxmox? |
|---|---|---|
| GUI development | Current Windows machine | No |
| Appliance smoke test | Disposable Proxmox VM | No, if kept separate from the host disks |
| USB boot test | Spare computer, USB drive | No |
| Full bare-metal test | Spare computer, internal disk | No, provided the correct disk is selected |

The USB image should initially have a **try mode** and an **install mode**.
Try mode runs Keel temporarily from memory and is useful for checking network,
hardware, and the GUI. Install mode writes Keel to the selected internal disk.
Try mode must not alter internal disks.

## Image contract

- amd64, UEFI first; legacy BIOS compatibility is a build acceptance target
- minimal Debian stable base with firmware and networking included
- Keel installed as `/usr/bin/keel` and enabled as `keel.service`
- persistent state at `/var/lib/keel`
- system and data storage remain separate
- first boot starts a local setup wizard; no destructive disk action is
  automatic
- the image contains the installer and required packages so installation can
  complete without an online package mirror
- Headscale/Tailscale enrollment is a post-network setup step, not a boot
  prerequisite

## Build a Ventoy test ISO from Windows

The current WSL-safe builder remasters Debian's signed official live image
without using a chroot. Open PowerShell in the Keel folder and run:

```powershell
wsl -d Debian -u root -- bash packaging/baremetal/remaster-official.sh
```

The first run downloads and verifies the Debian live ISO, so it can take some
time. The remaster follows Debian's current live amd64 **standard** ISO (today)
13.7.0). Set `KEEL_DEBIAN_LIVE_VERSION` only if you need a specific point
release. The result is `dist/baremetal/release/keel-amd64-install.iso`. Copy that single
file onto Ventoy's large data partition; do not extract it.

This first image is a non-destructive **Keel Live** smoke test. It boots
silently — blank, not splashed — straight into the Keel GUI on the machine's own screen: a full-screen
browser with no desktop, no login prompt, and no terminal. The same GUI is on
the network at port 9478, so a Windows browser can reach it too. Samba is
included so Windows File Explorer can open `\\<address>\files` with name `keel`
and password `keel` on the house network. That folder is temporary in try mode.

The reviewed Linux image can install onto one whole internal disk. It refuses
the disk it booted from and the USB it is running on, and it refuses a disk
that already has a mounted filesystem. The Install button stays off until the
image sets `KEEL_INSTALLER_ENABLED=1`. Confirming requires the exact words
`INSTALL KEEL`. That confirmation erases the selected disk. The Install page
shows one sentence and a bar while that runs; tool output stays in
`/run/keel/install.log`, not on the screen. When it finishes, the page asks
you to remove the USB and shows the sign-in token.

If the machine has no KMS-capable GPU, the kiosk cannot start; the boot falls
back to the text banner with the URL and admin token. A recovery console is on
`Ctrl+Alt+F3`.

## Fast rebuilds while developing

A full build unpacks 2.5 GB, runs apt, and recompresses with `xz`. None of that
changes when only Keel's own files changed, so set `KEEL_FAST=1` to keep the
prepared filesystem and replay just the copy steps, compressing with `zstd`:

```powershell
wsl -d Debian -u root -- env KEEL_FAST=1 bash packaging/baremetal/remaster-official.sh
```

The first build is always a full one; `KEEL_FAST=1` falls back to a full build
when there is no prepared filesystem yet.

**Use a full build when** you change `install-*-in-rootfs.sh`, the package set,
or anything that runs inside the chroot — those steps are skipped on the fast
path. **Release images must be full builds**, for `xz` compression and to prove
the whole thing still builds from scratch.

Override compression directly with `KEEL_SQUASHFS_COMP=xz|zstd` if needed.

## Development SSH

The image has no SSH unless you ask for it, and it never accepts passwords —
Debian live's built-in account has a password anyone can look up.

Create a key once, if you do not have one:

```powershell
ssh-keygen -t ed25519 -C keel-dev
```

Then build with `KEEL_SSH_PUBKEY` pointing at the public half, as a path the
Linux side can see:

```powershell
wsl -d Debian -u root -- env KEEL_SSH_PUBKEY=/mnt/c/Users/$env:USERNAME/.ssh/id_ed25519.pub bash packaging/baremetal/remaster-official.sh
```

That gives key-only `root` login, so `keelctl bootstrap --host root@<address>`
can push a rebuilt binary into a running machine in seconds instead of
rebuilding an ISO. Host keys are generated on first boot rather than baked in,
so two machines booting the same image do not share an identity.

Leave `KEEL_SSH_PUBKEY` unset for release images.

## Build host

The image is built on Debian or Ubuntu Linux (native, WSL2, or a disposable
Linux VM). The Windows developer tool will consume the resulting ISO/raw
image; it is not responsible for constructing a Linux root filesystem.

The current script builds the base live ISO and stages the Keel binary and
systemd unit. Run it with `--prepare` to validate and prepare the build
directory without producing an ISO, or without arguments to build the ISO.
It does not wipe disks or install to hardware. Those operations belong in a
separate, tested installer program with explicit confirmation and recovery
behavior.

The build host needs Linux, Rust/Cargo, and Debian `live-build`. Windows can
run the build through WSL2 or a disposable Linux VM; the output ISO can then
be written to USB by the future Windows GUI.

From Windows, the intended command will be:

```powershell
.
\packaging\baremetal\build-image.ps1
```

The launcher uses the Debian WSL2 distribution as root because `live-build`
needs to create a temporary chroot. Pass `-Distro Ubuntu` only for a different
build setup; Debian is the supported path.

Use `-Prepare` first if you only want to check the WSL2/build setup. The
script calls the Linux builder inside WSL2 and writes the result below
`dist/baremetal/release`. It does not write to a USB drive or any target disk.

If WSL2 is not available, install a Debian or Ubuntu distribution for WSL2 or
use a disposable Linux VM. We will not make users run this build command in
the finished product; it is a developer/release-engineering tool.

## Next implementation steps

1. Boot the installer in a disposable VM before using it on a real disk.
2. Add UEFI boot and ISO/raw-image CI smoke tests in a disposable VM.
3. Build the Windows `keelctl install-media` GUI flow with checksum and USB
   device confirmation.
4. Add a first-boot setup service that safely creates the admin credential and
   LAN binding before enabling the LAN-facing Keel service.
5. Add PXE bundle generation after the USB path is reliable.
