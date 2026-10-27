# Words we use in the GUI

Assume the reader has not heard NAS jargon. Prefer a plain proposal (Use this / Leave this) over a machine name.

Do not invent clever metaphors. Say what the thing is and what to do.

| We say | Means |
| --- | --- |
| Files | The name a PC uses to open those files. Same name at home and away |
| Disks | Where the files are stored |
| Apps | Programs that are installed. Open one when you need it |
| Away | The same files, when you are not at home |
| Rest | The computer stays on and apps stay closed. Away stays connected |
| Prepare wake | Turn on wake for the built-in network port. The computer stays on. Sleep waits until a Pi on Away can send the wake |
| Sleep when quiet | (Planned.) The EliteDesk may sleep or shut down to save watts. Away on that box pauses until it boots again. A Raspberry Pi stays on Away and can wake it. See wake-and-sleep.md |
| On another computer | Files stay on Proxmox or TrueNAS; keel does not wipe them |
| Disks in this machine | Files would live on drives in the keel box |
| 1 thing to look at | Home’s notice. Sea-glass. Fix it opens Sign in, then Disks. It does not erase |
| in this computer | How much the disks hold. The bar under a disk is how much of it is in use. It starts empty and fills. A long bar is filling up. Red means that disk says it is failing |
| Look around | Check one computer you name. Not a search of every machine |
| Open / closed | Like a computer: the app is on disk; running only when you need it |
| Away | Same files, not on the house Wi‑Fi. The overlay is how you reach them |
| Sign in | A name and password for the page. Settings on the Keel screen adds a person. The page records who signed in |
| Download | Save that file to the phone or computer you are using. A folder is opened, not downloaded |
| Try it on one disk | Ask one spinning disk in the USB box to sleep. If it is already asleep, Keel leaves the box to do that |
| Copy this computer | Save the Keel program, the people, and the settings onto the Nightly copy disk, in a folder of their own, apart from the files. A second copy goes on the Third copy disk when that disk is connected |
| Only the house and Away | The page and the folders answer on this network and on Away. The Keel screen still works. Other computers are refused |
| Lock the page | The same page at the Away name, with a lock the browser trusts. The password app asks for that address. The Keel screen stays as it is |
| Send to the phone | When something is red, one message goes to the free ntfy app. The phone must be on Away |
| Use this | Shared folder or files app a person should pick |
| Leave this | Disks for other programs, installer files, or the computer itself |

keel does not invent Proxmox names. After sign-in it reads the names that computer already uses, then applies a few rules (shared folder → Use this; installer disk / program disks → Leave this; a guest named like OpenCloud → family files). This lab needs that list while files stay on rok. Other installs mainly need it once, to keep personal files, then they use a share name.

Machine names (`vm-data`, `local`, `vmbr`, …) stay out of the heading by default. People who already know them can tick “Show the names that computer uses.” Use this / Leave this stays either way.

On the house network, Linux Keel currently uses File Explorer address `\\ip\files`, name `keel`, password `keel`. That is a temporary open LAN default, not a Proxmox password.

Short forms in the GUI (`lsblk`, RSS, …) are dotted links to Guide `#ids`. See terms.js.
