# Specialising down

keel is an appliance, so it may eventually carry only what a NAS needs. This
file is the research backing that, kept separate from `os.md` because none of it
is decided — `os.md` says what ships today, this says what we might cut later
and what it would cost.

The rule from `os.md` still governs: **speed comes from less software running,
not from lying.** Cutting must not cost accuracy. Debian's kernel, glibc and
systemd stay; what goes is the general-purpose userland around them.

## The single biggest lever: the browser

The kiosk is the largest thing we added, and most of it is avoidable.

| Stack | Installed | Download |
| --- | --- | --- |
| chromium (today) | 310 MB | 86 MB |
| libwpewebkit-2.0-1 + cog | ~120 MB | ~33 MB |

`cog` is a single-window WPE WebKit browser built for exactly our use: an
embedded device showing one web UI. Debian trixie has it at 0.18.4-1.

The larger prize is that Debian's cog ships `libcogplatform-drm.so`. The DRM
backend renders fullscreen directly to KMS, so a cog kiosk needs **no compositor
at all** — no cage, no wlroots, and no xwayland, which Debian's cage pulls in as
a hard dependency. Roughly 190 MB from the engine swap plus the compositor stack
on top.

Not free. WPE is WebKit, not Blink, so the GUI needs testing against it — low
risk for plain HTML/CSS/JS, real risk for anything Chrome-specific. The DRM
backend also means no VT switching and a rougher debugging story. Do this as a
deliberate swap with the GUI already working on chromium, not as a first move.

## Projects worth stealing from

- **[Talos Linux](https://www.talos.dev/)** — the closest philosophical match.
  No shell, no SSH, no package manager; read-only squashfs root; everything
  through an authenticated API. It is the proof that "the appliance never shows
  you a terminal" is a shipping product stance, not a wish. Steal the posture
  and the read-only root. Do not steal its scope: it exists to run Kubernetes.
- **Alpine** — musl + busybox, the biggest single cut available. Costs us
  Debian's firmware and kernel story, which `os.md` explicitly buys from Debian.
  Probably not worth it for us.
- **Buildroot** — names every package and builds from source. The endgame if
  keel ever leaves Debian, and the natural home for a true appliance image.
- **Yocto / OpenEmbedded** — heavier and layered; what balenaOS is built on.
  Worth knowing, probably too much machinery for one NAS.
- **balenaOS** — minimal host OS plus containers for apps. Shape is relevant,
  but `os.md` deliberately rejects containers for the catalog, so take only the
  host/app split idea.
- **Flatcar / CoreOS** — immutable root with A/B partition updates. Most
  relevant to our *install and update* story, which is still unwritten.
- **distroless** — the principle stated cleanly: ship the runtime, nothing else.

## Debian slimming we can do without leaving Debian

These are cheap and reversible, unlike the above.

- `dpkg path-exclude` in `/etc/dpkg/dpkg.cfg.d/` to drop `/usr/share/doc`,
  `/usr/share/man` and unused `/usr/share/locale`, keeping copyright files.
  This is what Ubuntu's minimal images do; locales are the largest share.
- `apt --no-install-recommends` everywhere (we already do this).
- `debootstrap --variant=minbase` for the built-from-scratch path.
- Firmware is the other fat package. `firmware-linux-nonfree` is large and most
  of it is for hardware the box does not have; `os.md` already wants a per-machine
  delta rather than the whole set.
- squashfs compression trades against `os.md`'s "fast boot" goal: we use `xz`
  for size, `zstd` decompresses markedly faster for a few percent more bytes.
  Measure before choosing.

## What not to cut

The kernel, glibc, systemd, `util-linux`, `smartmontools` and the storage tools.
`os.md`'s accuracy column depends on real `lsblk` and real SMART. A smaller image
that guesses at disk health is a worse product than a larger one that does not.

Sources: [Debian cog](https://packages.debian.org/trixie/cog),
[chromium](https://packages.debian.org/trixie/amd64/chromium),
[libwpewebkit](https://packages.debian.org/trixie/amd64/libwpewebkit-2.0-1),
[ReduceDebian](https://wiki.debian.org/ReduceDebian), [Talos](https://www.talos.dev/).
