#!/usr/bin/env bash
set -Eeuo pipefail

# Chroot-free Keel test-image builder.
#
# This remasters Debian's official hybrid live ISO instead of constructing a
# Debian filesystem from packages. It is useful on WSL2, whose chroot device
# handling is not sufficient for live-build package installation. No physical
# disk is read, partitioned, or written by this script.

ROOT_DIR="${KEEL_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
# Pin with KEEL_DEBIAN_LIVE_VERSION (e.g. 13.7.0). Otherwise use the current
# Debian live "standard" ISO listed in SHA256SUMS.
DEBIAN_VERSION="${KEEL_DEBIAN_LIVE_VERSION:-}"
BASE_URL="https://cdimage.debian.org/debian-cd/current-live/amd64/iso-hybrid"
WORK_DIR="${KEEL_REMASTER_DIR:-/var/tmp/keel-remaster}"
CACHE_DIR="${KEEL_ISO_CACHE_DIR:-/var/cache/keel-iso}"
OUT_DIR="${ROOT_DIR}/dist/baremetal/release"
ROOTFS_DIR="${WORK_DIR}/rootfs"
NEW_SQUASHFS="${WORK_DIR}/filesystem.squashfs"
NEW_SIZE="${WORK_DIR}/filesystem.size"
OUTPUT_ISO="${OUT_DIR}/keel-amd64.iso"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This builder must run on Linux or WSL2." >&2
  exit 2
fi

if [[ "$(id -u)" -ne 0 ]]; then
  echo "This builder needs root inside Linux to preserve image ownership." >&2
  exit 2
fi

for tool in curl md5sum sha256sum xorriso unsquashfs mksquashfs rsync; do
  command -v "${tool}" >/dev/null || {
    echo "Missing required build tool: ${tool}" >&2
    exit 2
  }
done

KEEL_BINARY="${ROOT_DIR}/target/release/keel"
if [[ ! -x "${KEEL_BINARY}" ]]; then
  echo "Missing Linux Keel binary: ${KEEL_BINARY}" >&2
  echo "Build it first with: cargo build --release" >&2
  exit 2
fi

mkdir -p "${CACHE_DIR}" "${OUT_DIR}"

curl --fail --location --output "${CACHE_DIR}/SHA256SUMS" \
  "${BASE_URL}/SHA256SUMS"

if [[ -n "${DEBIAN_VERSION}" ]]; then
  BASE_NAME="debian-live-${DEBIAN_VERSION}-amd64-standard.iso"
else
  BASE_NAME="$(awk '$2 ~ /^debian-live-.*-amd64-standard\.iso$/ { print $2; exit }' "${CACHE_DIR}/SHA256SUMS")"
fi

if [[ -z "${BASE_NAME}" ]]; then
  echo "Debian checksum list did not contain a live amd64 standard ISO." >&2
  exit 1
fi

BASE_ISO="${CACHE_DIR}/${BASE_NAME}"
echo "Using Debian live base: ${BASE_NAME}"

EXPECTED_SHA256="$(awk -v name="${BASE_NAME}" '$2 == name { print $1 }' "${CACHE_DIR}/SHA256SUMS")"
if [[ -z "${EXPECTED_SHA256}" ]]; then
  echo "Debian checksum list did not contain ${BASE_NAME}." >&2
  exit 1
fi

ACTUAL_SHA256=""
if [[ -f "${BASE_ISO}" ]]; then
  ACTUAL_SHA256="$(sha256sum "${BASE_ISO}" | awk '{print $1}')"
fi

if [[ "${ACTUAL_SHA256}" != "${EXPECTED_SHA256}" ]]; then
  echo "Downloading ${BASE_NAME} (resumes if interrupted)..."
  curl --fail --location --continue-at - \
    --output "${BASE_ISO}" "${BASE_URL}/${BASE_NAME}"
  ACTUAL_SHA256="$(sha256sum "${BASE_ISO}" | awk '{print $1}')"
fi

if [[ "${ACTUAL_SHA256}" != "${EXPECTED_SHA256}" ]]; then
  echo "Debian ISO checksum verification failed." >&2
  echo "Expected: ${EXPECTED_SHA256}" >&2
  echo "Actual:   ${ACTUAL_SHA256}" >&2
  exit 1
fi

# Fast rebuild. The slow half of this script is unsquashing 2.5 GB and running
# apt; neither changes when only Keel's own files change. KEEL_FAST=1 keeps the
# prepared rootfs and replays just the copy steps.
#
# A change to the *-in-rootfs.sh scripts, or to the package set, needs a full
# rebuild — those run only on the slow path. Release builds must be full.
# Beside the rootfs, never inside it: a marker in the tree would end up in the
# squashfs and in the size figure.
PREPARED_MARKER="${WORK_DIR}/.keel-rootfs-prepared"
FAST_REBUILD=0
if [[ "${KEEL_FAST:-0}" == "1" ]]; then
  if [[ -f "${PREPARED_MARKER}" ]]; then
    FAST_REBUILD=1
  else
    echo "KEEL_FAST=1 but no prepared rootfs yet; doing one full build first."
  fi
fi

case "${WORK_DIR}" in
  /var/tmp/keel-remaster|/var/tmp/keel-remaster-*)
    if [[ "${FAST_REBUILD}" -eq 1 ]]; then
      # Drop only what this run regenerates. The rootfs stays.
      rm -f -- "${NEW_SQUASHFS}" "${NEW_SIZE}"
    else
      rm -rf -- "${WORK_DIR}"
    fi
    ;;
  *)
    echo "Refusing to clean unexpected remaster directory: ${WORK_DIR}" >&2
    exit 2
    ;;
esac

mkdir -p "${WORK_DIR}"

# The boot configs are re-extracted every run, fast or not: they are tiny, and
# the edits below are applied to a pristine copy rather than stacked on an
# already-edited one.
echo "Extracting Debian live boot configuration..."
xorriso -osirrox on -indev "${BASE_ISO}" \
  -extract /md5sum.txt "${WORK_DIR}/md5sum.txt"
xorriso -osirrox on -indev "${BASE_ISO}" \
  -extract /isolinux/menu.cfg "${WORK_DIR}/isolinux-menu.cfg" \
  -extract /isolinux/live.cfg "${WORK_DIR}/isolinux-live.cfg" \
  -extract /boot/grub/grub.cfg "${WORK_DIR}/grub.cfg"

if [[ "${FAST_REBUILD}" -eq 1 ]]; then
  echo "Fast rebuild: reusing the prepared rootfs at ${ROOTFS_DIR}"
else
  echo "Extracting Debian live filesystem..."
  xorriso -osirrox on -indev "${BASE_ISO}" \
    -extract /live/filesystem.squashfs "${WORK_DIR}/original.squashfs"
  unsquashfs -processors "$(nproc)" -d "${ROOTFS_DIR}" \
    "${WORK_DIR}/original.squashfs"
fi

echo "Adding Keel and its first-boot services..."
install -D -m 0755 "${KEEL_BINARY}" "${ROOTFS_DIR}/usr/bin/keel"
install -D -m 0644 "${ROOT_DIR}/packaging/systemd/keel.service" \
  "${ROOTFS_DIR}/etc/systemd/system/keel.service"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/live/start-keel.sh" \
  "${ROOTFS_DIR}/usr/lib/keel/start-keel.sh"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/install-keel" \
  "${ROOTFS_DIR}/usr/lib/keel/install-keel"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/live/show-access.sh" \
  "${ROOTFS_DIR}/usr/lib/keel/show-access.sh"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/live/configure-network.sh" \
  "${ROOTFS_DIR}/usr/lib/keel/configure-network.sh"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/live/start-kiosk.sh" \
  "${ROOTFS_DIR}/usr/lib/keel/start-kiosk.sh"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/keel-kiosk.service" \
  "${ROOTFS_DIR}/etc/systemd/system/keel-kiosk.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/keel-live.conf" \
  "${ROOTFS_DIR}/etc/systemd/system/keel.service.d/live.conf"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/keel-console.service" \
  "${ROOTFS_DIR}/etc/systemd/system/keel-console.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/keel-network.service" \
  "${ROOTFS_DIR}/etc/systemd/system/keel-network.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/README.md" \
  "${ROOTFS_DIR}/usr/lib/keel/INSTALLATION.md"
mkdir -p "${ROOTFS_DIR}/usr/lib/keel/web" "${ROOTFS_DIR}/var/lib/keel"
rsync -a --delete "${ROOT_DIR}/web/" "${ROOTFS_DIR}/usr/lib/keel/web/"
find "${ROOTFS_DIR}/usr/lib/keel/web" -type d -exec chmod 0755 {} +
find "${ROOTFS_DIR}/usr/lib/keel/web" -type f -exec chmod 0644 {} +

mkdir -p "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants"
ln -sfn ../keel.service \
  "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/keel.service"
ln -sfn ../keel-console.service \
  "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/keel-console.service"
ln -sfn ../keel-network.service \
  "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/keel-network.service"
ln -sfn ../keel-kiosk.service \
  "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/keel-kiosk.service"
printf 'keel\n' > "${ROOTFS_DIR}/etc/hostname"

if [[ "${FAST_REBUILD}" -eq 1 ]]; then
  echo "Fast rebuild: skipping package installs (kiosk, Samba, SSH)."
else
  echo "Adding the appliance screen so the machine boots into Keel, not a shell..."
  bash "${ROOT_DIR}/packaging/baremetal/install-kiosk-in-rootfs.sh" "${ROOTFS_DIR}"

  echo "Adding Samba so Windows can open files on this computer..."
  bash "${ROOT_DIR}/packaging/baremetal/install-samba-in-rootfs.sh" "${ROOTFS_DIR}"

  # Opt-in, and only with a key. No key, no SSH — a release image should not
  # carry a developer's access.
  if [[ -n "${KEEL_SSH_PUBKEY:-}" ]]; then
    echo "Adding key-only SSH for development..."
    bash "${ROOT_DIR}/packaging/baremetal/install-ssh-in-rootfs.sh" \
      "${ROOTFS_DIR}" "${KEEL_SSH_PUBKEY}"
  else
    echo "No KEEL_SSH_PUBKEY set; building without SSH."
  fi

  touch "${PREPARED_MARKER}"
fi

install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/smb.conf" \
  "${ROOTFS_DIR}/etc/samba/smb.conf"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/smb-shares.conf" \
  "${ROOTFS_DIR}/var/lib/keel/smb-shares.conf"
mkdir -p "${ROOTFS_DIR}/var/lib/keel/files"
cat > "${ROOTFS_DIR}/var/lib/keel/files/README.txt" <<'EOF'
This folder is on the Keel computer.
From Windows File Explorer open \\<keel-address>\files
Name: keel
Password: keel
On USB try-mode this folder is temporary. It is emptied when you shut down.
EOF
chmod 0666 "${ROOTFS_DIR}/var/lib/keel/files/README.txt"

# This is a try-from-USB image. Remove Debian's installer entries so the boot
# menu cannot lead a tester into an unrelated internal-disk installation.
sed -i '/^include install\.cfg$/d; s/^menu title .*/menu title Keel Live/' \
  "${WORK_DIR}/isolinux-menu.cfg"
sed -i \
  -e 's/Live system (amd64 fail-safe mode)/Keel Live (fail-safe mode)/' \
  -e 's/Live system (amd64)/Keel Live (safe USB test)/' \
  "${WORK_DIR}/isolinux-live.cfg"
sed -i \
  -e '/^# Installer (if any)$/,/^fi$/d' \
  -e 's/Live system (amd64 fail-safe mode)/Keel Live (fail-safe mode)/' \
  -e 's/Live system (amd64)/Keel Live (safe USB test)/' \
  "${WORK_DIR}/grub.cfg"

# Boot silently. Scrolling kernel and systemd text is the other half of "this
# appliance never shows you a terminal".
#
# This is not a Plymouth splash. Plymouth draws from the initramfs, and this
# builder reuses the stock Debian /live/initrd.img rather than regenerating it,
# so installing Plymouth into the squashfs would produce a package that never
# runs. Doing it properly means update-initramfs inside the rootfs and mapping
# the result over /live/initrd.img — worth doing, but it risks an unbootable
# ISO and belongs in its own change. Until then the screen is simply blank from
# the boot menu until the kiosk paints.
#
# nomodeset marks Debian live's fail-safe entry, which stays verbose on purpose
# for recovery.
# Debian's live GRUB menu waits for a keypress forever. An appliance that a
# newbie plugs in must reach its own screen without a keyboard attached at all,
# so pick the default entry and move on. The menu is still there for the few
# seconds someone needs to choose fail-safe.
sed -i '/^[[:space:]]*set timeout=/d; /^[[:space:]]*set default=/d' "${WORK_DIR}/grub.cfg"
printf 'set default=0\nset timeout=5\n' | cat - "${WORK_DIR}/grub.cfg" \
  > "${WORK_DIR}/grub.cfg.new"
mv "${WORK_DIR}/grub.cfg.new" "${WORK_DIR}/grub.cfg"

QUIET_ARGS='quiet loglevel=3 systemd.show_status=false vt.global_cursor_default=0'
sed -i "/^[[:space:]]*append /{ /nomodeset/!{ /loglevel=3/!s/\$/ ${QUIET_ARGS}/ } }" \
  "${WORK_DIR}/isolinux-live.cfg"
sed -i "/linux.*\/live\/vmlinuz/{ /nomodeset/!{ /loglevel=3/!s/\$/ ${QUIET_ARGS}/ } }" \
  "${WORK_DIR}/grub.cfg"

# xz is the smallest and by far the slowest; on a 2.5 GB rootfs it dominates
# the build. zstd costs a few percent in size and compresses several times
# faster, which is the right trade while iterating. Debian's kernel has
# squashfs zstd support, so both boot.
SQUASHFS_COMP="${KEEL_SQUASHFS_COMP:-}"
if [[ -z "${SQUASHFS_COMP}" ]]; then
  if [[ "${FAST_REBUILD}" -eq 1 ]]; then
    SQUASHFS_COMP=zstd
  else
    SQUASHFS_COMP=xz
  fi
fi

echo "Compressing the Keel live filesystem (${SQUASHFS_COMP})..."
mksquashfs "${ROOTFS_DIR}" "${NEW_SQUASHFS}" -comp "${SQUASHFS_COMP}" -b 1M -noappend
du -sx --block-size=1 "${ROOTFS_DIR}" | cut -f1 > "${NEW_SIZE}"

# Keep Debian Live's optional boot-menu integrity check useful after replacing
# the filesystem payload.
sed -i \
  '\| \./live/filesystem\.squashfs$|d; \| \./live/filesystem\.size$|d; \| \./isolinux/menu\.cfg$|d; \| \./isolinux/live\.cfg$|d; \| \./boot/grub/grub\.cfg$|d' \
  "${WORK_DIR}/md5sum.txt"
printf '%s  ./live/filesystem.squashfs\n' "$(md5sum "${NEW_SQUASHFS}" | awk '{print $1}')" \
  >> "${WORK_DIR}/md5sum.txt"
printf '%s  ./live/filesystem.size\n' "$(md5sum "${NEW_SIZE}" | awk '{print $1}')" \
  >> "${WORK_DIR}/md5sum.txt"
printf '%s  ./isolinux/menu.cfg\n' "$(md5sum "${WORK_DIR}/isolinux-menu.cfg" | awk '{print $1}')" \
  >> "${WORK_DIR}/md5sum.txt"
printf '%s  ./isolinux/live.cfg\n' "$(md5sum "${WORK_DIR}/isolinux-live.cfg" | awk '{print $1}')" \
  >> "${WORK_DIR}/md5sum.txt"
printf '%s  ./boot/grub/grub.cfg\n' "$(md5sum "${WORK_DIR}/grub.cfg" | awk '{print $1}')" \
  >> "${WORK_DIR}/md5sum.txt"

echo "Writing the Ventoy-compatible hybrid ISO..."
rm -f -- "${OUTPUT_ISO}"
xorriso -indev "${BASE_ISO}" -outdev "${OUTPUT_ISO}" \
  -boot_image any replay \
  -map "${NEW_SQUASHFS}" /live/filesystem.squashfs \
  -map "${NEW_SIZE}" /live/filesystem.size \
  -map "${WORK_DIR}/md5sum.txt" /md5sum.txt \
  -map "${WORK_DIR}/isolinux-menu.cfg" /isolinux/menu.cfg \
  -map "${WORK_DIR}/isolinux-live.cfg" /isolinux/live.cfg \
  -map "${WORK_DIR}/grub.cfg" /boot/grub/grub.cfg \
  -volid KEEL_LIVE

(
  cd "${OUT_DIR}"
  sha256sum keel-amd64.iso > keel-amd64.iso.sha256
)

echo "Created: ${OUTPUT_ISO}"
echo "Checksum: ${OUTPUT_ISO}.sha256"
echo "Copy the ISO file to the large Ventoy data partition."
echo "No physical disk was modified."
