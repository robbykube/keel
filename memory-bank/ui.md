# UI intents (from Cerebro)

Keel’s admin face is a browser. Interaction and layout follow Cerebro
(`E:\Personal Projects\cerebro`), which already settled these on real screens.
Keel keeps its own look (sea-glass on deep water). Do not copy Cerebro’s warm
yellow `#f3c548` as the NAS accent.

Source: Cerebro `productContext.md` “How it should feel” and current window
conventions (15 Sep 2026).

## Window

- **The window is the frame.** The page does not grow a document scrollbar.
  Chrome stays put. One scroll region per view; a list/detail split scrolls
  each side inside itself. Why boxes start closed and scroll in themselves.
- **The sidebar never scrolls.** Primary rows stay visible. Extra destinations
  open from a short menu to the right (Cerebro: Work / Research / Library),
  not a long stacked list. Guide and Settings sit above Notes. Notes stays at
  the bottom of the sidebar.

Keel’s mapping in `web/chrome.js`:

- Everyday: Home, Files, Apps, Away
- Menu to the right, **This computer**: Disks and Repair. Install is only on
  the live USB image (`can_install`), never in the installed app. The row names
  the page you are on when it belongs to that menu.
- Support: Guide, Settings, Sign in
- Notes at the bottom, then health and a Power button (icon + word). Its menu
  opens upward: Sign out (only for a browser session), Restart, Shut down.
  Restart and Shut down confirm first.
- Sign in hides from the sidebar once signed in (`/api/auth/session`).
- Rest stays a Home link, not a sidebar row
- **One window.** Not a browser tab per subsystem. Dashboard, Shares, Overlay,
  Repair, Guide, Notes share one chrome.
- **Content fills the width.** Prose keeps a reading measure. Cards sit 16px
  apart. A form sits beside its list when there is room (Shares, Pools).
- **Save is never below the fold.**

## Save-button rule

- A control with no pending typed state (tick, slider stop, single choice)
  saves on change.
- Anything typed into keeps an explicit button.
- Bulk or hard-to-undo work still confirms (wipe, install, forget a host).
- A failed save repaints from the source of truth. Autosave writes only what
  changed.

## Words and help

- Help assumes the reader knows nothing, and says why each thing is done.
- Errors name the fix a person can do.
- Plain page names. If a part name is needed, it is a small label, not the
  heading (Cerebro: “Chains · Cortex”). Keel already prefers Use this / Leave
  this over machine names.
- Short forms (`lsblk`, RSS) stay Guide links.

## Colour and status (intent, not palette)

- Accent is for interactive highlight and live/ok work. Keel’s accent stays
  sea-glass, not Cerebro yellow.
- An optional service that is not running reads **grey**, not red. Red is a
  failure that needs Repair. “Overlay not enrolled yet” is grey.
- Scroll bars are thin and themed (track quiet, thumb warm/accent on hover).
- Keyboard focus is always visible; pointer clicks do not need a ring.

## Two surfaces, one UI

The same web UI is reached two ways, and neither is a lesser version:

- **The Keel box’s own screen.** A full-screen browser on `127.0.0.1:9478`, no
  desktop around it. Plug in a monitor and keyboard and the appliance is usable
  by someone who has never seen a terminal. This is the first-run path.
- **This PC’s browser** against the box over LAN or overlay. The everyday path
  once the box is set up and probably headless.

Design for the local screen too: it has a mouse and keyboard but no window
manager, no address bar, no tabs, and no back button outside the page. Anything
that assumes the user can open a second tab, read a URL, or copy a token out of
somewhere else is broken on that surface.

SSH and the recovery console (`Ctrl+Alt+F3`) are recovery, not setup. Files open
in File Explorer (`\\ip\share`); the GUI does not replace Explorer.
