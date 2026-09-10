# Lab: Proxmox node rok

Single-node datacenter. All guests seen so far are **LXC 100–111**, mostly helper-script installs. No QEMU VMs in the Datacenter summary screenshot.

## Node

- Name: `rok`
- Disk ~81% (`local` ~81%; `local-lvm` ~0%; `vm-data` ~61%; `localnetwork` present)
- RAM ~39%, CPU low at capture; uptime ~1d 16h

## Guests

| ID | Name | Role | Seen |
| --- | --- | --- | --- |
| 100 | tailscale-vpn | Overlay / remote path | running |
| 101 | scanopy | analytics | stopped |
| 102 | technitiumdns | LAN DNS | running |
| 103 | vaultwarden | passwords | running |
| 104 | opencloud | files/cloud | running, disk hot (~76%) |
| 105 | jitsi-meet | video | running |
| 106 | immich | photos | stopped |
| 107 | proxmox-backup-server | backup | stopped |
| 108 | nginxproxymanager | reverse proxy / TLS | running |
| 109 | papra | documents | stopped |
| 110 | proxmox-mail-gateway | mail | running |
| 111 | open-archiver | archive | running |

- **Stopped LXCs are still the catalog map** (have ≠ run on a future keel box).
- **OpenCloud (104)** is the file app on rok today; keel does not replace it there.
- **`local` is 81% full.** No NAS data there, and no keel OS install on that disk.
- Intranet: Technitium, NPM, Tailscale stay on rok until a second box exists.

## What this means for keel

Do **not** install keel OS on the `rok` host. It already runs the homelab. `local` is ~81% full. Replacing Proxmox with keel would take Tailscale, DNS, Vaultwarden, OpenCloud, NPM, mail with it.

Develop the control plane here (Windows). First real Linux keel is a **second machine**, or a **disposable VM** on `vm-data` with **no** HBA passthrough (GUI/OS experiments only — not the family NAS).

keel may use rok/TrueNAS as a **foreign pool** (NFS/SMB) instead of reimaging them. See storage-backends.md.
