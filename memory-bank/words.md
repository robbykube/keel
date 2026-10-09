# Words we use in the GUI

Assume the reader has not heard NAS jargon. Prefer a plain proposal (Use this / Leave this) over a machine name.

Do not invent clever metaphors. Say what the thing is and what to do.

| We say | Means |
| --- | --- |
| Pool | Where the files are stored |
| Share | The name a PC uses to open those files. Same name at home and away |
| On another computer | Files stay on Proxmox or TrueNAS; keel does not wipe them |
| Disks in this machine | Files would live on drives in the keel box |
| Look around | Check one computer you name. Not a search of every machine |
| Open / closed | Like a computer: the app is on disk; running only when you need it |
| Away | Same files, not on the house Wi‑Fi. The overlay is how you reach them |
| Use this | Shared folder or files app a person should pick |
| Leave this | Disks for other programs, installer files, or the computer itself |

keel does not invent Proxmox names. After sign-in it reads the names that computer already uses, then applies a few rules (shared folder → Use this; installer disk / program disks → Leave this; a guest named like OpenCloud → family files). This lab needs that list while files stay on rok. Other installs mainly need it once, to keep personal files, then they use a share name.

Machine names (`vm-data`, `local`, `vmbr`, …) stay out of the heading by default. People who already know them can tick “Show the names that computer uses.” Use this / Leave this stays either way.

On the house network, Linux Keel currently uses File Explorer address `\\ip\files`, name `keel`, password `keel`. That is a temporary open LAN default, not a Proxmox password.

Short forms in the GUI (`lsblk`, RSS, …) are dotted links to Guide `#ids`. See terms.js.
