# Guide UX and power

Match the AI Suite habit: **explain at the point of action**, plus a **Guide** that is one click from every screen.

## Idle vs stopped vs installed

- **Host** stays on (no S3). That is what “idle, not stopped” means for the box.
- **Installed:** full rok catalog, including guests that were stopped on rok.
- **Running while idle (floor):** **Tailscale** (or the overlay we ship) **plus keel**. That path is how you open files and the GUI when you are not at home.
- **Everything else** (OpenCloud, Immich, Jitsi, DNS, NPM, …) is **stopped** until Active. Still on disk, not running, not using RAM/CPU.

Remote **Active** over Tailscale does **not** mean every guest at full tilt. Active means you can open an app. Shares should still work on the overlay while idle if the file door (Samba) is part of the floor or can be opened on demand without bringing the whole catalog up.

Auto-apply only when `safe_while_working` is true. Never S3 the host. Never stop Tailscale. Never uninstall the catalog.
