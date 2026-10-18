# Wake and sleep (planned)

Sleep is not built. Default power story stays **Rest**: host on, apps parked, Away up.
Settings **Prepare wake** arms the built-in wired port (`ethtool wol g`, remembered in `keel-wake.service`) and shows its address. It does not sleep or shut down the computer.
This page is the optional deeper watt path we agreed (2026-10-17).

## Hardware split

| Machine | Role |
| --- | --- |
| **HP EliteDesk 800 G2 Mini** | Real Keel NAS: disks, shares, nightly copy, catalog apps |
| **Raspberry Pi** | Always-on sidekick: Away (Tailscale) stays up; can wake the EliteDesk |

The Pi is **not** the family NAS. Keel’s ship path is amd64 today; the Pi does not need a full Keel image for wake. Tailscale plus a small wake action is enough. Full Keel on the Pi is optional later if we want the same GUI there.

EliteDesk still wins for USB bulk disks (Yottamaster), copies, and apps. The Pi wins on idle watts (~2–5 W) so Away can survive while the desk sleeps or is soft-off.

## Two Settings postures (planned)

1. **Stay reachable** (today’s Rest) — Away always works; park apps and spin down idle disks. Default.
2. **Sleep when quiet** (optional) — EliteDesk sleeps or shuts down to save more watts. Away on the NAS is **paused**, not uninstalled. When the desk boots again, Keel and Tailscale start and Away returns. The UI must say that clearly.

Sleep is never the only energy plan. Host S3 alone (no always-on helper) stays rejected: nothing on Away can wake the box, and files-from-away die for the whole sleep window with no recovery path.

## Soft-off vs button

After **Shut down**, the EliteDesk is usually **soft-off (S5)**: it looks off, but wall power still feeds a little standby to the board and NIC. The power button works; **Wake-on-LAN can also work** if BIOS allows it. Soft-off is not the same as unplugged.

| State | Wake with |
| --- | --- |
| Sleep (S3) | Button, WoL (if enabled) |
| Soft-off (Shut down) | Button, WoL (if enabled) |
| No wall power | Power back, then button **or** BIOS auto-on |

## EliteDesk BIOS (800 G2 Mini)

Confirmed as a normal path on this model (HP business Mini):

1. **Wake On LAN** → **Boot to Hard Drive**
2. **S5 Maximum Power Savings** → **Disable** (if left on, soft-off often kills NIC standby and WoL fails)
3. **After Power Loss** → **Power On** (so outages match the Pi: electricity back → desk boots)

Use the built-in wired LAN. USB/Wi‑Fi adapters usually do not do WoL. Some G2 units are flaky after long soft-off; BIOS update helps. vPro/AMT is a backup only if that SKU has it.

## Who wakes whom

| Event | Who acts |
| --- | --- |
| Power cut, then power back | EliteDesk BIOS **After Power Loss → Power On** (Pi also boots by itself) |
| Intentional sleep / Shut down; wake from Away | Pi (on Tailscale) sends a **magic packet** to the EliteDesk MAC |
| Intentional wake while on the house network | Any PC/phone on the LAN can send WoL; Pi not required |
| Alternate Away wake | Smart plug + BIOS power-on-after-AC (harder cut; fine after clean Shut down) |

The Pi cannot wake the desk during a blackout (no power). Once wall power returns, BIOS auto-on is the outage path; the Pi’s job is **Away wake while the desk chose to be off**.

## Wake helpers ranked

1. **Pi on Away + WoL** — clean for sleep or soft-off; keeps trust inside Tailscale
2. **Smart plug** controllable from Away (cloud app, or Shelly/HA on Tailscale) — simple for full off if BIOS restores power on AC
3. **Router WoL page** — only if that router is already reachable on Away; do not build Keel around one router brand
4. **House-only WoL** — fine footnote when someone is home

## Product rules when this lands

- Default remains Rest. Sleep/soft-off is opt-in in Settings.
- Guide/Why copy: Away on the NAS pauses while asleep; Away returns after boot; wake from Away needs the Pi (or another house-side sender).
- Nightly copy needs either the desk awake for that window, an RTC/BIOS power-on schedule, or “do not sleep before the copy finishes.”
- Do not expose WoL or router admin to the public internet.
- Advisor keeps “never S3 the host **alone** to save a few watts”; with a Pi floor, optional sleep is a different product choice.

## Not in this plan

- Replacing the EliteDesk with the Pi as the NAS
- Shipping an arm64 Keel image only to enable wake (unnecessary for the sidekick role)
- Making sleep the default energy strategy
