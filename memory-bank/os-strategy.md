# Decision: we do not build our own OS

**Status:** decided, 2026-09-18. Revisit only if Debian's ZFS packaging dies or
we ship our own hardware.

## The decision

keel builds its **own image, own control plane, and own update mechanism** on
Debian's kernel, glibc and systemd. We do not write our own init, libc, or
package manager, and we do not fork the kernel.

To a user this *is* keel's own OS: they never see Debian, never see a terminal,
never run apt. "Our own OS" is a property of the product surface, not of who
compiled libc.

## Why not further down

Four different things get called "building your own OS":

1. Curated image on a mainstream distro — where we are.
2. Own image built from source (Buildroot) — plausible, see `size.md`.
3. Own userland: own init, libc, package manager.
4. Own kernel fork.

3 and 4 carry a permanent tax: owning a userland means owning its security
updates forever. Debian supplies a security team at no cost. One person cannot
match that, and a NAS holding a family's photos is exactly the wrong place to
find out.

## The evidence from this field

- **TrueNAS ran this experiment and reversed it.** iXsystems moved from a
  self-maintained FreeBSD appliance base to Debian-based SCALE, citing hardware
  support and ecosystem, and by late 2023 put all feature engineering there.
  The best-resourced company in our exact space moved *toward* a mainstream
  distro, not away.
- **OpenMediaVault is Debian plus a web UI** — structurally what keel is.
- **Unraid is Slackware-based.** Nobody serious in NAS builds from scratch.

The pattern: curate a distro, own the control plane.

## The keel-specific reason

**ZFS.** It is out-of-tree and needs a module built against every kernel.
Debian packages that for us. Owning the kernel means owning that build forever,
for every bump, on a storage appliance where a failed module build means the
pool does not import. That alone rules out level 4.

## What we own instead

The differentiators are not the packages:

- the GUI and the control plane (`keel`)
- the image and what is *not* in it (`size.md`)
- **the update mechanism** (`updates.md`) — the real appliance property, and
  the one we have not built

Size is a number. An appliance that cannot safely take a security update is a
dead product.
