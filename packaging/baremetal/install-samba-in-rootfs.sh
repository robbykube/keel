#!/usr/bin/env bash
set -Eeuo pipefail

# Install Samba into an unsquashed Debian live rootfs. No physical disk is written.

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

if [[ -r /etc/resolv.conf ]]; then
  mkdir -p "${ROOTFS_DIR}/etc"
  cp /etc/resolv.conf "${ROOTFS_DIR}/etc/resolv.conf"
fi

# Debian Live's apt list includes the USB medium, which is not mounted in this
# chroot. Use the network Debian mirrors only for the install, then restore.
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
chroot "${ROOTFS_DIR}" apt-get install -y --no-install-recommends samba
chroot "${ROOTFS_DIR}" apt-get clean
rm -rf "${ROOTFS_DIR}/var/lib/apt/lists/"*

if [[ -f "${APT_LIST}.keel-bak" ]]; then
  mv "${APT_LIST}.keel-bak" "${APT_LIST}"
fi

if ! chroot "${ROOTFS_DIR}" id keel >/dev/null 2>&1; then
  chroot "${ROOTFS_DIR}" useradd --system --home /var/lib/keel \
    --shell /usr/sbin/nologin --user-group keel
fi

printf 'keel\nkeel\n' | chroot "${ROOTFS_DIR}" smbpasswd -a keel -s

mkdir -p "${ROOTFS_DIR}/var/lib/keel/files"
chroot "${ROOTFS_DIR}" chown keel:keel /var/lib/keel/files
chmod 0777 "${ROOTFS_DIR}/var/lib/keel/files"

rm -f "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/nmbd.service"
if [[ -e "${ROOTFS_DIR}/lib/systemd/system/smbd.service" ]]; then
  ln -sfn /lib/systemd/system/smbd.service \
    "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/smbd.service"
elif [[ -e "${ROOTFS_DIR}/usr/lib/systemd/system/smbd.service" ]]; then
  ln -sfn /usr/lib/systemd/system/smbd.service \
    "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/smbd.service"
fi
