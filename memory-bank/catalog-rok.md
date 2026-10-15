# Ship-with catalog (rok)

Come with as much of this lab as possible. **Have** ≠ **run**. Stopped-on-rok guests still ship. Idle **runs Tailscale** (the path to files and the GUI when you are not at home). Active starts guests **on demand**. The advisor may park them again if they go quiet.

| Role | rok ID | Idle |
| --- | --- | --- |
| Tailscale | 100 | **Running** — overlay + file path from away |
| Technitium DNS | 102 | Stopped until Active |
| Vaultwarden | 103 | Stopped until Active |
| Jitsi | 105 | Stopped until Active |
| Immich | 106 | Stopped until Active |
| Nginx Proxy Manager | 108 | Stopped until Active |
| Papra | 109 | Stopped until Active |
| Proxmox Mail Gateway | 110 | Stopped until Active |
| Open Archiver | 111 | Stopped until Active |

On keel they are **packages + systemd**, not LXCs. rok IDs are the lab map only. CPU save = stopped units, not container tax.

OpenCloud (104), Proxmox Backup Server (107), and Scanopy (101) stay facts about rok. They are not Keel apps. Files, Nightly copy, and Third copy cover those jobs. A map, if one is needed, is Keel’s own page.
