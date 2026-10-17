# Guide UX and power

Match the AI Suite habit: **explain at the point of action**, plus a **Guide** that is one click from every screen.

## Idle vs stopped vs installed

- **Host** stays on by default (no S3). That is what “idle, not stopped” means for the box under **Rest**.
- **Installed:** full rok catalog, including guests that were stopped on rok.
- **Running while idle (floor):** **Tailscale** (or the overlay we ship) **plus keel**. That path is how you open files and the GUI when you are not at home.
- **Everything else** (Immich, Jitsi, DNS, NPM, …) is **stopped** until Active. Still on disk, not running, not using RAM/CPU. OpenCloud, Proxmox Backup Server, and Scanopy are not in this list. Files is the file manager. Nightly copy and Third copy are the backup. Disks already remembers the computers you looked at.

Remote **Active** over Tailscale does **not** mean every guest at full tilt. Active means you can open an app. Shares should still work on the overlay while idle if the file door (Samba) is part of the floor or can be opened on demand without bringing the whole catalog up.

Auto-apply only when `safe_while_working` is true. Never S3 the host **alone** to save watts (that kills Away with no wake path). Never stop Tailscale while Rest is the posture. Never uninstall the catalog.

## Optional sleep (planned)

A later Settings choice may let the EliteDesk sleep or soft-off while a **Raspberry Pi** stays on Away and can send Wake-on-LAN. Away on the NAS pauses until the desk boots again; then Keel and Tailscale start as usual. Outages use EliteDesk BIOS **After Power Loss → Power On**. Full plan: [wake-and-sleep.md](wake-and-sleep.md).
