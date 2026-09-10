# Intranet and install

keel is installed onto a machine that **already has Windows or Linux**. Bare-metal PXE is out of scope until that path is explicit.

## Install paths

| From (your PC) | Onto (NAS box) | Transport |
| --- | --- | --- |
| Windows | Linux | OpenSSH, push Linux binary + systemd |
| Windows | Windows | OpenSSH or WinRM, Windows service |
| Linux | Linux | SSH |

The Windows GUI you use for development is **not** the binary that runs on a Linux NAS. CI or WSL produces the matching OS/arch build.

Linux remains the real storage OS (ZFS/mdadm/SMB stack). Windows-as-host is a management + SMB path (NTFS/ReFS/Storage Spaces), not a clone of the Linux pool model.

## Why intranet breaks NAS

These fail more often than the daemon itself:

1. **Layer-2 isolation** — guest Wi-Fi, AP client isolation, IoT VLAN. Phone/PC cannot see the NAS even if both “have internet.”
2. **Wrong bridge / NAT** — Proxmox VM on NAT or a management-only vmbr. UI loads from the hypervisor VLAN; SMB clients on LAN cannot hit 445.
3. **Firewall stack** — Proxmox DC + VM firewall + guest nftables/Windows Firewall. One of them drops 9478, 445, 22.
4. **Discovery vs reachability** — mDNS/WS-Discovery/NetBIOS do not cross VLANs or many APs. A working TCP IP:port still fails “browse.”
5. **Hairpin / split DNS** — `nas.example.com` resolves to WAN; LAN clients hairpin poorly.
6. **IPv6 vs IPv4** — UI on v6, SMB on v4, or the reverse.
7. **Docker/LXC published ports** — host port mapped, but SMB needs host network or a dedicated IP. Nested NAT kills SMB performance and broadcasts.
8. **Multiple subnets without a router ACL** — allowed ICMP, blocked 445.

## Workarounds keel should own (product, not hope)

- **Never rely on discovery alone.** Always show bind address, port, and a copy-paste `\\ip\share` for LAN **and** the overlay address (`\\100.x.x.x\share` or MagicDNS) so files work away from home.
- **Listen on a chosen interface**, default “LAN”, not `0.0.0.0` onto a WAN NIC. Warn if the only route is a public IP.
- **Health that tests the client path**: from the admin browser, fetch `/api/health`; separately document SMB with a one-click “this host reached keel on X.”
- **Static IP + hostname** in the GUI as day-one setup, not optional.
- **Port matrix** in the UI: 9478 (GUI), 445 (SMB), 22 (install). Red if we cannot bind.
- **Tailscale (or the overlay we ship)** is how you reach **files** away from home, not only the GUI. Samba/shares must work on that path. LAN first at home; overlay when you are not on the house network. Do not sleep the host or this path dies.
- Never rely on opening ports on the public internet for family files.
- **Install wizard asks:** same subnet as your PC? If no, print the VLAN/firewall checklist instead of hanging on scan.

## This lab (rok)

See lab-rok.md. Overlay is LXC 100 Tailscale; names are Technitium (102); HTTP front door is NPM (108). OpenCloud (104) is the file app already.

- Give a future keel guest a NIC on the **same bridge the PCs use**, not NAT.
- NAS data: not on `local` (already ~81%). Passthrough or `vm-data` / new disks.
- Do not run keel on the Proxmox host; `rok` is busy and must stay up for remote nap-cancel.
- Open 22/9478/445 toward LAN + Tailscale; leave 8006 as Proxmox only.
- Hairpin: if NPM publishes a public name, Technitium should return the **LAN** IP for that name on-site.
