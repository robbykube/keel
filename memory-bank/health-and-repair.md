# Health and repair (local, before any AI)

Troubleshooting is a **first-class screen**, not Settings archaeology and not Chef/Loom.

## Goal

See the failure in one sentence, see why, press **Fix** when it is safe. Seconds, not a hunt.

## Rules

- Home names one thing to look at, in sea-glass. Red on Repair is a disk that is actually failing. Free space and SMART are not read yet, so Home shows how much the disks hold, not how full they are.
- Every issue has **evidence** (unit, port, SMART, `lsblk`).
- **Fix** only if `safe: true` (restart a unit, rebind a port, rescan disks). Destructive work (wipe pool) is never one click.
- Known playbook on Linux: restart samba, restart Tailscale, open 445/9478 toward LAN **and** the overlay, rescan disks, start a parked unit that should be up in Active. Same buttons, real systemctl later.
- Journal tail for the failed unit sits **on that row**, not in a log app.
- AI Suite is later: optional bundle. Repair must work with Loom off.

## Setup

Few questions (hostname, pool disks, admin, join Tailscale so files work away). Defaults + Why this. Do not learn the OS through Settings.
