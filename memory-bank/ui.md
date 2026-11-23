# UI intents (from Cerebro)

Keel’s admin face is a browser. Interaction and layout follow Cerebro
(`E:\Personal Projects\cerebro`), which already settled these on real screens.
Keel uses a warm neutral surface with a restrained brass accent: calm enough
for an appliance, but clearer and more welcoming than a generic admin console.

Source: Cerebro `productContext.md` “How it should feel” and current window
conventions (15 Sep 2026).

## Window

- **The window is the frame.** The page does not grow a document scrollbar.
  Chrome stays put. One scroll region per view, and its scrollbar sits on the
  right edge of the window. A list/detail split scrolls each side inside
  itself. Why boxes start closed and scroll in themselves.
- **The sidebar never scrolls.** Primary rows stay visible. Extra destinations
  open from a short menu to the right (Cerebro: Work / Research / Library),
  not a long stacked list. Guide and Settings sit above Notes. Notes stays at
  the bottom of the sidebar.

Keel’s mapping in `web/chrome.js`:

- Everyday: Home, Files, Apps, Away. Home asks “What will you use Keel for?”
  (chips, under the three cards) until apps are chosen; Apps can add more.
  Home is the family page, in this order: a greeting, one notice, then the
  Files, Away and Disks cards, then the address other computers use (one line
  with Copy), then the app chooser. No person’s name. No Family/Admin switch.
- **This computer** is a sidebar title, with Disks, Devices, Bluetooth and Repair under it.
  Install is only on the live USB image (`can_install`), never in the installed
  app. The row names the page you are on when it belongs to that menu.
- Support: Guide stays. Settings and Sign in follow the session. Sign in is
  there until you are signed in; Settings appears after that.
- Notes at the bottom, then the health line, then a Power button (icon + word).
  The health line is drawn by `chrome.js` on every page from `/api/issues`:
  a dot and “All good”, “1 thing to look at” (or n things), or “Needs
  fixing”, with the version on the right; it links to Repair. Every page has
  `#health`, so the sidebar sits the same on every page. Its menu
  opens upward. Signed in: Sign out (only for a browser session), Restart,
  Shut down. Until then the menu is Sign in, so someone in the house cannot
  shut the computer down from the page. Restart and Shut down confirm first.
- Sign in hides from the sidebar once signed in (`/api/auth/session`).
- Rest stays a Home link, not a sidebar row
- **One window.** Not a browser tab per subsystem. Dashboard, Shares, Overlay,
  Repair, Guide, Notes share one chrome.
- **Content fills the width.** Prose keeps a reading measure. Cards sit 16px
  apart. A form sits beside its list when there is room (Shares, Pools).
- **Save is never below the fold.**
- An install or a download shows one sentence and a bar. The sentence names the step. The bar moves through that step and stops just short of the next one until the step is finished. Tool output stays off the page.
- On a small screen the top bar stays put and does not scroll. Home, Files, and Away share that row. Disks, Repair, Guide, Settings, and Notes open from More. Page columns stack. The page content is the one scroll, including the folder list and the file list on Files.

## Save-button rule

- A control with no pending typed state (tick, slider stop, single choice)
  saves on change.
- Anything typed into keeps an explicit button.
- Bulk or hard-to-undo work still confirms (wipe, install, forget a host).
- A failed save repaints from the source of truth. Autosave writes only what
  changed.

## Words and help

- **The page says what to do; `?` says why.** A heading or control gets one
  short line at most. Anything longer (why, how it works, what it touches)
  goes behind a `?` button: `<button class="help" data-tip="…">` in HTML, or
  `keelHelp(text)` in script. Pressing it opens a bubble; anywhere else or
  Escape closes it. It works by tap on the Keel screen. No hover-only help.
- Do not write the same sentence twice on a page (lede and empty state).
- A raw message from the computer (systemd, HTTP codes, tool output) is never
  the sentence on a card. The card says what happened and what to press; the
  raw text goes behind `?` (“What the computer said”) or into Logs.
- Help assumes the reader knows nothing, and says why each thing is done.
- Errors name the fix a person can do.
- Plain page names. If a part name is needed, it is a small label, not the
  heading (Cerebro: “Chains · Cortex”). Keel already prefers Use this / Leave
  this over machine names.
- Short forms (`lsblk`, RSS) stay Guide links.

## Design pieces

The look is one list of names in `web/style.css` (`:root`). A page does not
invent its own font size, gap, radius, or color.

- **Color:** `--bg`, `--panel`, `--text`, `--muted`, `--accent` (brass),
  `--copper` for labels, `--success`, `--warn`, `--red` for a real failure.
  Light or dark lives on Settings, remembered in this browser. Both themes
  retain the same contrast, spacing, and status meaning.
- **Space:** `--space-1` (4px) through `--space-6` (28px). Gaps and stacks
  use these. Control padding is `--pad-control`. Card padding is `--pad-card`.
  Both are built from that scale.
- **Type:** `--font` and `--font-mono`. Sizes are `--text-2xs` through
  `--text-display`. Eyebrows (`h2`, kickers, table heads) share `--text-2xs`
  and `--track-label`. A card title (`h2.home-title`) uses `--text-xl` and is
  left out of that eyebrow rule.
- **Shape:** `--radius` for panels, `--radius-control` for buttons and fields,
  `--shadow-card` for raised surfaces.
- **Power** stays the sidebar button (icon and word). Signed in, its menu is
  Sign out, Restart, and Shut down. Until then the menu is Sign in. Do not
  draw a second power control on a page.

## Opening an app

- Open is one press. It goes to the Keel window (`/open.html?app=<id>`),
  which starts the app if needed and then shows it. No address to copy, no
  password to type where Keel made the account.
- A running app's card shows Open and Close side by side. Close stops the
  app until Open; a restart does not start it again. Running is what the
  computer reports, not what the page remembers.
- The window keeps a slim bar: Back, the app's name, New tab, Full screen.
  It is the same on the Keel screen and on a PC.
- An app card shows a password only where the person must type one (Mail),
  as its own field with Copy, for signed-in people. Never inside a sentence.

## Lists and rows

- Each row carries its own action (Disks: “Give it a job…”). A destructive
  action is never the only or the first button on a row; the erase is inside
  the form that asks for the disk name.
- A disk is named by its model, with the device name (`sda`) small under it.
- Sizes use one format everywhere: `keelSize` (1000-based, like the label on
  a disk).
- An empty list shows one sentence and the button that fills it (Files: “Choose
  a disk”), and hides tools that need something in the list.
- On a phone, a wide table uses `class="stack"` and becomes stacked rows; no
  button may be cut off at the edge.
- `[hidden]` always hides, even on a grid or flex form.

## Colour and status (intent, not palette)

- Accent is for interactive highlight and live/ok work. Healthy status uses
  green, attention uses brass, and only a real repair uses red.
- An optional service that is not running, or an app that did not start, reads **grey** (`pill idle`), not red. Red is a
  failure that needs Repair. “Overlay not enrolled yet” is grey.
- Scroll bars are thin and themed (track quiet, thumb warm/accent on hover).
- Keyboard focus is always visible; pointer clicks do not need a ring.

## Two surfaces, one UI

The same web UI is reached two ways, and neither is a lesser version:

- **The Keel box’s own screen.** A full-screen browser on `127.0.0.1:9478`, no
  desktop around it. A monitor and a pointer are enough. Choosing a box that
  needs letters shows a keyboard on the page, and it goes away when you leave
  the box. A keyboard plugged in later still types. This is the first-run path.
- **This PC’s browser** against the box over LAN or overlay. The everyday path
  once the box is set up and probably headless.

Design for the local screen too: it has a pointer, and no window manager, no
address bar, no tabs, and no back button outside the page. The page shows a
keyboard when a box needs letters. Anything that assumes the user can open a
second tab, read a URL, or copy a token out of somewhere else is broken on
that surface.

SSH and the recovery console (`Ctrl+Alt+F3`) are recovery, not setup. Files open
in File Explorer (`\\ip\share`); the GUI does not replace Explorer.
