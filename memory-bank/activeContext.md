# Active context

What Keel is today, as of v0.1.99 (2026-11-24). History is in
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
| | **Apps** | Chosen apps first, each with Open. Open shows the app in the Keel window (below). Then Add another, then Names and the front door. The name you own is in Settings. |
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

### Open: an app opens like a program

Open on an app card goes to the **Keel window**, `/open.html?app=<id>`. The
window starts the app if it is closed (one sentence and a bar), then shows it
under a slim bar: Back, the app's name, New tab, Full screen. A failure says
“did not start. Try again, or open Logs”, with what the computer said behind
`?`.

- **Address with no DNS.** Photos, Watch, Papers and Mail each have a port on
  the front door, on Keel's own addresses: 127.0.0.1 (the Keel screen), the
  house address, and the Away address when there is one. Photos 9481, Watch
  9482, Papers 9483, Mail 9484 (`launch.rs` `APPS`; range 9481–9489, opened
  by the firewall). The app opens on the address the browser used for Keel.
  The `<app>.<name you own>` names stay, for outside the house.
- **Keel signs you in** where Keel made the account: Photos today. The window
  asks `POST /api/apps/{id}/launch` (signed-in only) for a one-time ticket
  (one app, one minute). The browser opens
  `http://<keel>:<port>/keel-open/<ticket>`; the front door sends that one path
  to Keel's local listener (127.0.0.1:9477, beside recordings); Keel signs in
  to the app on this computer and passes the app's own cookies back (no
  Domain, not Secure), then sends the browser to `/`. The Photos password
  stays in `/var/lib/immich/account.pass` and never reaches the page.
- **Video and Passwords keep their locked name** (`https://video.<name>`,
  `https://passwords.<name>`): a camera, a microphone and the vault only work
  on a locked page. On the Keel screen (127.0.0.1 counts as locked) they open
  in the window; on another computer the whole page goes to them.
- The front door strips Keel's own `keel_session` cookie before an app sees a
  request, and drops `X-Frame-Options` so the app can sit in the window.
- **Keel signs in to Photos, Watch, Papers and Mail.**
  - Photos: Keel's account (`/var/lib/immich/account.pass`); Immich's own
    cookies are passed back.
  - Papers: on a first install Keel makes the house account
    (`papers@<name>`) and keeps it in `/var/lib/keel/signin/papers.json`.
    Papers is told every Keel address (`APP_BASE_URL=http://<house>:9483`,
    `TRUSTED_ORIGINS` = Keel screen, house, Away, the name), or its sign-in
    cookie would not stick on the port. A Papers started with other addresses
    (older Keel, or the house address changed) is closed and opened again by
    the window, once.
  - Watch: on a first install Keel finishes Jellyfin's setup (account
    `house`, one library over Media) and keeps it in
    `/var/lib/keel/signin/watch.json`. Watch keeps its sign-in in the browser,
    so the hand-off is a tiny page on Watch's port that writes
    `jellyfin_credentials` and opens `/web/`.
  - Mail (full): Mail's page asks the browser for a password, so its port has
    a gate: Open gives a `keel_gate_mail` cookie (12 h, in memory), the front
    door asks `/keel-gate/mail` before each request (`forward_auth`), and Keel
    answers with Mail's password for Mail only. Without the cookie the port
    says “Open Mail from Keel's Apps page.” Light Mail (SnappyMail) signs in
    with your own email account, so Keel does not.
  - **Sign in once.** Where Keel did not make the account (Watch or Papers
    you set up yourself), the window asks once for the name or email and
    password, checks them with the app (`POST /api/apps/{id}/remember`), and
    keeps them only if the app accepts them (root, 0600, in
    `/var/lib/keel/signin/`). “Open without” skips it.
  - Passwords never signs in for you: the vault's master password stays
    yours. Video has no sign-in.
- Photos and Mail cards no longer put a password in their sentence; it was
  readable without signing in.
- **Close on every app.** A running app's card shows Open and Close.
  `POST /api/apps/{id}/stop` (signed-in) stops the app's service with
  `systemctl disable --now` and its containers with `podman stop`, so it stays
  closed after a restart until Open (which enables it again). Program,
  settings and data stay. Video asks once (“End the call and close”).
- **Running is asked of the computer.** `GET /api/apps/running` reads
  `podman ps` (and keel-galene). The card's own note lives in /run and is gone
  after a restart, while an enabled app starts again with the computer, so
  cards used to say closed, with no Close, for an app that was running.

Built and tested here against pretend apps (each hand-off, one-time tickets,
the Mail gate, Sign in once with a wrong and a right password, cookies and
Watch's browser sign-in inside the window, the Keel screen's keyboard over the
form). Not yet run with the real programs: the Jellyfin setup calls and its
browser sign-in format, Papra's sign-up and sign-in, and the front door's new
config are written from their documentation.

### Machines

- **EliteDesk** (amd64, 7.8 GB): installed 0.1.88 on 2026-11-23.
- **Raspberry Pi 4** (0.9 GB as Linux sees it, 800×480 DSI screen): 0.1.79
  on 2026-11-19. Memory is the constant constraint there; see releases.md
  0.1.75–0.1.87.
- **This Windows PC**: development. `cargo run` serves the page with pretend
  disks. Files cannot be served from Windows.

## Latest change (v0.1.99)

After Open finishes, each app is stopped and started again by its own service, so it stays on when the setup job ends. Close turns that service off. Logs names whether Photos, Mail, Passwords, Papers, Watch, and Video are on. Not yet installed.

## Previous (v0.1.98)

Open rewrites the front door's helper before it reloads, so a computer that already had its apps can answer on the app ports. Settings can check, save, and forget the Photos, Papers, and Watch sign-ins. Logs includes Mail. Not yet installed.

## Previous (v0.1.97)

Open on an app shows it in the Keel window, and signs in where Keel made the account. Close is on every running app. Not yet installed.

## Previous (v0.1.96)

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

1. Install v0.1.99 on the EliteDesk from Settings. It is not on that computer until then.
2. Core path before more apps: pick a disk for Files on the EliteDesk, open
   `\\ip\files` from Windows, connect Away, until Home shows no notice.
3. `GET /api/shares` still returns the folder password without a session.
4. The Pi: find what fills `/run` (Repair hover text, Logs “What takes the
   temporary space”).
5. Galène can go back into the package now that 0.1.88+ accepts an apps folder.
