#!/usr/bin/env bash
set -Eeuo pipefail

# Tools the disk installer runs while the machine is still on the live USB.
# Debian's live image does not include them. The installed copy is that same
# filesystem, so GRUB has to be here too: install-keel runs grub-install inside
# the copy. No physical disk is written.

ROOTFS_DIR="${1:-}"
if [[ -z "${ROOTFS_DIR}" || ! -d "${ROOTFS_DIR}" ]]; then
  echo "Usage: $0 /path/to/rootfs" >&2
  exit 2
fi

cleanup() {
  for m in proc sys dev/pts dev; do
    if mountpoint -q "${ROOTFS_DIR}/${m}"; then
      umount -l "${ROOTFS_DIR}/${m}" || true
    fi
  done
}
trap cleanup EXIT

mkdir -p "${ROOTFS_DIR}/proc" "${ROOTFS_DIR}/sys" "${ROOTFS_DIR}/dev/pts"
mount -t proc proc "${ROOTFS_DIR}/proc"
mount -t sysfs sysfs "${ROOTFS_DIR}/sys"
mount --bind /dev "${ROOTFS_DIR}/dev"
mount --bind /dev/pts "${ROOTFS_DIR}/dev/pts" || true

if [[ -e "${ROOTFS_DIR}/etc/resolv.conf" || -L "${ROOTFS_DIR}/etc/resolv.conf" ]]; then
  cp -a "${ROOTFS_DIR}/etc/resolv.conf" "${ROOTFS_DIR}/etc/resolv.conf.keel-bak"
fi
if [[ -r /etc/resolv.conf ]]; then
  mkdir -p "${ROOTFS_DIR}/etc"
  cp /etc/resolv.conf "${ROOTFS_DIR}/etc/resolv.conf"
fi

APT_LIST="${ROOTFS_DIR}/etc/apt/sources.list"
if [[ -f "${APT_LIST}" ]]; then
  cp "${APT_LIST}" "${APT_LIST}.keel-bak"
fi
cat > "${APT_LIST}" <<'EOF'
deb http://deb.debian.org/debian/ trixie main contrib non-free-firmware
deb http://security.debian.org/debian-security trixie-security main contrib non-free-firmware
EOF

export DEBIAN_FRONTEND=noninteractive
chroot "${ROOTFS_DIR}" apt-get update
# grub-pc-bin has the BIOS modules without grub-pc's "which disk?" question.
# grub-efi-amd64 has the EFI files and /etc/default/grub that update-grub reads.
chroot "${ROOTFS_DIR}" apt-get install -y --no-install-recommends \
  dosfstools \
  e2fsprogs \
  squashfs-tools \
  parted \
  grub-efi-amd64 \
  grub-pc-bin
chroot "${ROOTFS_DIR}" apt-get clean
rm -rf "${ROOTFS_DIR}/var/lib/apt/lists/"*

if [[ -f "${APT_LIST}.keel-bak" ]]; then
  mv "${APT_LIST}.keel-bak" "${APT_LIST}"
fi
if [[ -e "${ROOTFS_DIR}/etc/resolv.conf.keel-bak" || -L "${ROOTFS_DIR}/etc/resolv.conf.keel-bak" ]]; then
  mv "${ROOTFS_DIR}/etc/resolv.conf.keel-bak" "${ROOTFS_DIR}/etc/resolv.conf"
fi
