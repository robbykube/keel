# Active context

What Keel is today, as of v0.1.94 (2026-11-23). History is in
[releases.md](releases.md).

## The app today

Keel is one Rust program (`keel`, port 9478) that serves a browser page and a
JSON API, on a Debian-based image for a whole computer (amd64 ISO, Raspberry
Pi image). The same page is reached on the Keel computer's own screen (kiosk,
loopback, trusted) and from any browser in the house or on Away.

### Pages

Sidebar on a computer screen; a top bar with Home, Files, Apps, Away and
**More** below 960 px.

| Group | Page | What it does |
| --- | --- | --- |
| Everyday | **Home** | Greeting, one notice (same count as Repair), then three cards: Files, Away, Disks. Under them: the address other computers use (one line, Copy) and, until apps are chosen, “What will you use Keel for?” as chips. Rest link and Details at the bottom. |
| | **Files** | The shared folders (`\\ip\files`). Browse, make folders, move, copy, download, view pictures. Sections group folders. Who opens the folders (SMB name/password) and extra share names are in closed boxes. Empty until a disk has the Files job. |
| | **Apps** | Chosen apps first, each with Open. Then Add another, then Names and the front door. The name you own is in Settings. |
| | **Away** | Install and connect the overlay (Tailscale or a server you run); Lock the page. Below that, “From outside the house”: whether people outside can reach the apps, and Ask the router. |
| This computer | **Disks** | Each disk with model, size, fill bar and what it holds; per-row “Give it a job…” (erases after typing the disk name). Idle sleep, nightly Copies, Copy this computer / bring it back. Another computer (Proxmox/TrueNAS/WebDAV) read-only. |
| | **Devices** | What is on the house network; mark known, watch important ones. |
| | **Bluetooth** | Look, Pair, Disconnect, Forget. |
| | **Repair** | Issues (red “Needs fixing”, yellow “Worth a look”), safe Fix buttons, Send to the phone (ntfy). |
| | **Install** | Only on the live USB. |
| Support | **Guide**, **Logs**, **Settings** (Updates from GitHub, the name you own, Light or full, People, When it is quiet, Appearance), **Sign in** until signed in | |
| Bottom | **Notes**, health line, **Power** | |

### Apps (each opens only when asked)

Passwords (Vaultwarden), Video (Galène light / Jitsi full), Photos (Keel's own
light viewer / Immich full), Watch (Jellyfin), Papers (Papra), Mail
(SnappyMail light / Thunderbird full). Names (Technitium) and the front door
(Caddy) start on their own and gate the apps. Settings → Light or full picks a
version; Keel suggests light under 2.5 GB of memory. See
[systemPatterns.md](systemPatterns.md) for how each runs.

### Machines

- **EliteDesk** (amd64, 7.8 GB): installed 0.1.88 on 2026-11-23.
- **Raspberry Pi 4** (0.9 GB as Linux sees it, 800×480 DSI screen): 0.1.79
  on 2026-11-19. Memory is the constant constraint there; see releases.md
  0.1.75–0.1.87.
- **This Windows PC**: development. `cargo run` serves the page with pretend
  disks. Files cannot be served from Windows.

## Latest change (v0.1.96)

“From outside the house” is on Away, under Connect and Lock the page. Apps no longer has it. Not yet installed.

## Previous (v0.1.95)

Apps opens on the apps. The name you own is a field in Settings, under Updates. Names sits below the apps. If the name is not saved, the apps say so and point to Settings.

## Previous (v0.1.94)

A usability pass on the page. Nothing in the API changed meaning.

- **Help is a ?.** Long explanations moved off the page into small `?`
  buttons (`button.help[data-tip]`, or `keelHelp(text)` in script; bubble in
  `chrome.js`). Pages keep one short line and the action. See ui.md “Words
  and help”.
- **Home** puts Files, Away and Disks first. The address line and the app
  chooser come after them. The notice is built from `/api/issues`, so Home,
  Repair and the sidebar say the same count in the same words.
- **One health line** above Power on every page (`keelShowHealth` in
  `chrome.js`): “All good”, “1 thing to look at”, or “Needs fixing”, plus the
  version; it links to Repair. Every page now has `#health`, so the sidebar no
  longer shifts between pages.
- **One size format** (`keelSize`, 1000-based) for Home and Disks.
- **Disks** rows lead with the model and give each disk its own “Give it a
  job…”. The erase form stays hidden until then (`[hidden]` now always hides).
- **Errors name a fix.** A stopped app card is grey with “did not start. Try
  again, or open Logs”, and the raw message sits behind `?`. GitHub 403/429
  says to connect GitHub or try later.
- **Files** shows one empty state with “Choose a disk”, hides New folder and
  New section until there are folders, and on small screens lists folders
  first and tools last.
- **Phones**: wide tables (`table.stack`) become stacked rows, so no button is
  cut off. Ticks and radios are 18 px; `?` has a larger touch area.
- The pretend-disk issue on Windows now reads “Practice copy: the disks are
  pretend”.

Checked in Chromium at 1440×900, 1280×720, 800×480 and 390×844 against a
Linux build with pretend data. Not yet seen on the EliteDesk or the Pi.

## Next

1. Install v0.1.96 on the EliteDesk from Settings. It is not on that computer until then.
2. Core path before more apps: pick a disk for Files on the EliteDesk, open
   `\\ip\files` from Windows, connect Away, until Home shows no notice.
3. `GET /api/shares` still returns the folder password without a session.
4. The Pi: find what fills `/run` (Repair hover text, Logs “What takes the
   temporary space”).
5. Galène can go back into the package now that 0.1.88+ accepts an apps folder.
