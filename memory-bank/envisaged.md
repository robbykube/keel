# Envisaged (not v0)

Stay a curated appliance. Do not become TrueNAS, Unraid CA, or a second Proxmox.

## Pillars worth building

1. **Power advisor (while working)** — busy vs parked: CPU, disk I/O, Jitsi conferences, Immich jobs, backup window. Explain each save. Auto only if safe.
2. **Warm-on-demand catalog** — hitting `photos.*` starts Immich; empty Jitsi stays down. Idle floor remains Tailscale + keel so files still open from away.
3. **Intranet doctor** — one page: LAN IP:port, Tailscale IP, Technitium name, NPM name, hairpin test, 22/445/9478. The usual homelab failure, made obvious.
4. **Integrity without a science project** — snapshots on a clock, scrub on a clock, SMART as health, one “yesterday’s files” restore. Not every ZFS knob.
5. **Backup as a window** — the nightly copy runs, then stops. Same pattern as the power advisor. Proxmox Backup Server is not a Keel app.
6. **Shares that explain ACLs** — guest / user / group in words, copy-paste `\\lan-ip\share` **and** `\\tailscale-ip\share` (or MagicDNS). Same door at home and away.
7. **Watt + latency budget on the dashboard** — RAPL or a cheap meter if we have it; daemon RSS; API timing. Speed and energy as one product.
8. **One Update** — Omarchy-style: catalog + keel + host patches, rollback if the pool is fine.
9. **Presence** — phone on LAN or Tailscale → raise a small Active set; gone → decay back to floor. Not a full home-automation suite.
10. **Pool placement guard** — refuse NAS data on 81%-full `local`; want passthrough or a dedicated vdev.
11. **Overlay as the away file path** — tight Tailscale (or successor) relationship: join on first boot, show overlay address next to every share, Samba on that NIC. Swap the brand, keep the job.
12. **Optional sleep + Pi wake** — Rest stays default. Settings may offer sleep/soft-off on the EliteDesk while a Raspberry Pi keeps Away and sends WoL. Outages: BIOS power-on after AC. Plan: [wake-and-sleep.md](wake-and-sleep.md).

## AI Suite (later)

Optional bundle to Loom/Chef. Local Repair must work first. See health-and-repair.md.

## Explicitly not keel

Parked product thoughts live in `E:\Personal Projects\ideas` (video brand/SaaS, thin files app). They are not Keel work.

- Hypervisor UI (rok stays Proxmox)
- Plugin bazaar
- Second OpenCloud (SMB is the file path; a web files UI is an idea note, not the catalog)
- Host S3 **alone** as the energy strategy (no always-on Away helper / wake path). Optional sleep *with* a Pi wake floor is planned — see wake-and-sleep.md.
- TUI admin
- Running Chef/Loom on the NAS
