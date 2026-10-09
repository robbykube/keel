# Product context

NAS products either hide storage behind a slow app store (CasaOS) or expose every ZFS knob (TrueNAS). Proxmox is the UX we want: one web app, obvious objects, no sitting at the server.

keel exists so we can cut work that does not serve “store files, share them, stay fast”: extra runtimes, heavy frontends, TUI admin, optional plugins for basics.

## How it should work

1. Install the appliance (or push keel onto Debian).
2. First boot: hostname, pool disks, admin, join Tailscale (or the overlay we ship) so files work away. No settings tour.
3. Dashboard is green/red. Repair is a sentence plus a safe Fix. Do not live in Settings.

## GUI-first rule

Normal installation, app installation, storage setup, overlay enrollment,
updates, repair, and power policy must be available from the browser GUI. SSH,
the shell, and the VM console are bootstrap and recovery paths, not the normal
product experience. Keel should feel like a modern operating system with a
clear setup flow, app catalog, notifications, and reversible actions.

## UX goals

Interaction and layout follow Cerebro (`E:\Personal Projects\cerebro`). Look
stays Keel: sea-glass on deep water. Details in [ui.md](ui.md).

- The GUI **is** the OS face: pretty, quiet, dense. Not a default admin template.
- **The window is the frame.** The page does not grow a document scrollbar. Chrome stays put. The sidebar never scrolls; extra pages open from a short menu. Notes stays at the bottom of the sidebar. Wide pages use two panes so they fill the window.
- **Save-button rule** (Cerebro): ticks and choices save on change; typed fields keep a button; hard-to-undo still confirms; a failed save repaints from the source of truth.
- Why boxes start **closed** so they do not eat the fold. If opened, they scroll inside themselves.
- Help assumes the reader knows nothing. Errors name the fix. Optional/not-running is grey, not red.
- Small JS, no SPA framework tax in v0
- Information density over marketing cards
- Same explanation bar as Cerebro / the AI Suite: a always-on **Guide**, **Notes** for ideas, and **Why this?** at consequential actions (what, why, what changes, how to undo)
- Headings are a proposal in plain words (Use this / Leave this). Machine names stay in Why or the Guide. No clever metaphors.
- Power: keel works like an OS. Installed ≠ running. Open an app when you need it. Idle = Tailscale floor; Active = start what is asked for; a **power advisor** still closes unused work while you are using the box. Never S3 the host.
- **Away:** Tailscale (or the same kind of private path if we switch) is how you open files when you are not at home. Same share. The box stays on so that path stays up. Overlay is not an optional extra.
