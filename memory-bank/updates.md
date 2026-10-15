# Updates: A/B slots and rollback

**Status:** signed program updates are implemented. Whole-disk A/B slots are
still this design, and they are not built.

What works now:

- GitHub Actions publishes `keel-linux-amd64-vX.Y.Z.tar.gz`, `SHA256SUMS`, and
  `SHA256SUMS.sig` when a `vX.Y.Z` tag is pushed. The tag must match
  `Cargo.toml`.
- The private signing key lives only in the GitHub Actions secret
  `KEEL_RELEASE_SIGNING_KEY`. The matching public key is
  `packaging/updates/release-public.hex` and is built into the `keel` binary.
  Do not put the private key in git, memory-bank, or chat.
- The GitHub repository is private. Settings **Connect GitHub** uses device
  flow: a code on the Keel screen, approve on a phone or PC. The access token
  is saved to `KEEL_DATA/github.token` and is never shown again.
  `KEEL_GITHUB_TOKEN` in the environment still works. The GitHub app id is
  public (`packaging/updates/github-client-id`, `KEEL_GITHUB_CLIENT_ID`, or
  `KEEL_DATA/github.client_id`). That token is not a signing key and must not
  be committed. Making the repository public removes the need for it.
- Settings checks the latest stable GitHub release and verifies the signature
  before it will offer Install update. A push to main is not an update. The
  box only counts a signed `vX.Y.Z` release. Private release files are
  downloaded from the GitHub API as raw bytes, not as JSON.
- Install saves the current program, pages, and `/var/lib/keel` settings, then
  switches to the new program. The new program is marked runnable (`0755`).
  Unpack alone leaves a normal file, systemd still says the restart worked,
  and the health check then restores the previous version. If the new version
  does not answer `/api/health`, or the power goes out before that answer, the
  boot service `keel-update-rollback.service` restores the saved copy.
- Settings written by a newer Keel than this binary understands stop startup
  instead of being guessed.

What is still the design below, not code: RAUC, a second root slot, and
updating the operating system itself. The GitHub package is the Keel program.
It is not a second root filesystem. Do not unpack an update over the running
system by hand.

This is the largest missing piece in the product, larger than image size.

## The rule

A power cut in the middle of an update must never produce a machine that does
not boot. That is the whole design constraint, and it is why the update is
never "unpack files over the running system."

**Do not hand-roll the slot selection and rollback.** The embedded update
literature is unanimous and the failure mode is bricked devices. Debian trixie
ships `rauc` 1.13 ("safe and secure software updates for embedded Linux"), which
handles slot state, atomic writes and bootloader interaction, and supports both
GRUB and systemd-boot. Alternatives are SWUpdate and Mender; libostree solves a
different shape (file-level deployments rather than whole slots).

RAUC is the fit: flexible slot layout, no fixed partition assumptions, and it
does not require the embedded u-boot world.

## Partition layout on the system disk

Storage pools are **not** here. `os.md` keeps system and data storage separate;
pools live on their own disks and no update touches them.

| Partition | Size | Contents |
| --- | --- | --- |
| ESP (FAT32) | 512 MB | bootloader + both slots' kernels and initrds |
| root A | ~4 GB | read-only root image |
| root B | ~4 GB | read-only root image |
| keel-data | rest | `/var/lib/keel`: config, shares, tokens, logs |

Two roots roughly double root usage — the main cost of A/B, and the reason
`size.md` matters more once this lands. 4 GB slots leave headroom over the
current image even with chromium in it.

`keel-data` survives every update and every rollback. It is the only writable
partition on the system disk; both roots are read-only, which is the Talos
property worth stealing.

GRUB rather than systemd-boot, because the README commits to legacy BIOS
compatibility as a build acceptance target and systemd-boot is UEFI-only.

## The update flow

1. keel fetches the image and verifies **checksum and signature** before writing
   anything. We already checksum the ISO at build time; this needs a signing key
   whose private half is not in the repo.
2. RAUC writes it to the **inactive** slot. The running system is untouched, so
   an interrupted write costs nothing.
3. The inactive slot is marked *try once*, with a boot attempt counter.
4. Reboot into the new slot.
5. The new slot must **prove itself**: keel starts, answers `/api/health`, and
   the pools import. Only then is the slot marked good.
6. If it does not prove itself — failed boot, or no confirmation within the
   attempt limit — the bootloader falls back to the previous slot automatically.

Step 5 is the part people skip. A slot that boots but whose pools do not import
is a failed update, and only keel can tell the bootloader that.

## The trap: config outlives the rollback

Rollback restores the root, not `keel-data`. If a new keel writes a config the
old keel cannot parse, rolling back gives a booting system with an unreadable
config — a failure that looks like data loss to the owner.

So: **version the config schema**, have keel refuse to start on a schema newer
than it understands rather than guessing, and snapshot the config into the slot
before an update so a rollback can restore the matching pair.

## What the GUI owes the user

Per `ui.md`, this has to be legible to someone who has never updated a server:
what changed, that it can be undone, and what happens if the power goes out
(nothing). After a rollback the GUI must say a rollback happened and why — a
silent revert is worse than a failed update.

## Open questions

- Where do images come from, and who signs them?
- Delta updates or whole slots? Whole slots first; deltas are an optimisation.
- Does the catalog (`os.md`) update on this cycle or its own? Probably its own —
  a photo app should not gate a kernel security fix.
