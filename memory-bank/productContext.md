# Product context

NAS products either hide storage behind a slow app store (CasaOS) or expose every ZFS knob (TrueNAS). Proxmox is the UX we want: one web app, obvious objects, no sitting at the server.

keel exists so we can cut work that does not serve “store files, share them, stay fast”: extra runtimes, heavy frontends, TUI admin, optional plugins for basics.

## How it should work

1. Install the appliance (or push keel onto Debian).
2. First boot: hostname, pool disks, admin, join Tailscale (or the overlay we ship) so files work away. No settings tour.
3. Dashboard is green/red. Repair is a sentence plus a safe Fix. Do not live in Settings.

## UX goals

- The GUI **is** the OS face: pretty, quiet, dense. Sea-glass on deep water, not a default admin template.
- **The window is the frame.** The page does not scroll. Chrome stays put. Wide pages use two panes (Guide, Pools, Shares, Repair) so they fill the window instead of a thin left column.
- Why boxes start **closed** so they do not eat the fold. If opened, they scroll inside themselves.
- Small JS, no SPA framework tax in v0
- Information density over marketing cards
- Same explanation bar as the AI Suite: a always-on **Guide**, **Notes** for ideas, and **Why this?** at consequential actions (what, why, what changes, how to undo)
- Power: keel works like an OS. Installed ≠ running. Open an app when you need it. Idle = Tailscale floor; Active = start what is asked for; a **power advisor** still closes unused work while you are using the box. Never S3 the host.
- **Away:** Tailscale (or the same kind of private path if we switch) is how you open files when you are not at home. Same share. The box stays on so that path stays up. Overlay is not an optional extra.
