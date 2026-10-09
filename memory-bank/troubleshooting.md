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
