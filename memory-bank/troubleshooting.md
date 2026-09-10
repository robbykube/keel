# Troubleshooting

## GUI troubleshooting

Use Repair in the app: sentence + Fix. Do not start in Settings. AI Suite is not required.

## cargo run fails to bind 9478

Another process owns the port. Set `KEEL_PORT` or stop the other listener.

## Empty or mock disks on Windows

Expected. Inventory is mock unless `cfg(target_os = "linux")`.

## GUI or SMB unreachable on the LAN

Check same L2/VLAN as the client, Proxmox NIC on the LAN bridge (not NAT), and firewalls for 9478 (GUI) and 445 (SMB). mDNS failing is not a daemon failure if `http://ip:9478` works. See intranet.md.

## lsblk missing on Linux

Install `util-linux`. keel falls back to an error disk entry rather than crashing.
