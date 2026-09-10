# keel

Opinionated NAS appliance with a **web GUI**, not a TUI.

The point of building our own is control over **efficiency and speed**: one small daemon, a dense browser UI, short path from disk to share. No Electron, no desktop session on the NAS, no plugin bazaar in v1.

## Principles

1. Appliance, not a ricing distro. Defaults should be enough.
2. Admin is a browser on the LAN, like Proxmox.
3. Measure: daemon RSS, API latency, page weight.
4. Linux is the host. Windows is for UI/API development with mock disks.

## Run

```bash
cargo run
```

Open http://127.0.0.1:9478

## Status

v0: dashboard, disk inventory, stub pools/shares. Storage mutations come next.
