#!/usr/bin/env bash
set -Eeuo pipefail

# Install the appliance screen into an unsquashed Debian live rootfs: a Wayland
# kiosk compositor, a browser, a boot splash, and the unprivileged user they run
# as. No physical disk is written.
#
# This is the package cost of a graphical appliance. It is roughly half a
# gigabyte on a root that is otherwise deliberately small; see memory-bank/os.md
# for why that trade was made.

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
# curl is not guaranteed in Debian's live "standard" squashfs, and
# start-kiosk.sh waits on /api/health with it. Name it rather than assume it.
chroot "${ROOTFS_DIR}" apt-get install -y --no-install-recommends \
  cage \
  chromium \
  libgl1-mesa-dri \
  fonts-dejavu-core \
  curl
chroot "${ROOTFS_DIR}" apt-get clean
rm -rf "${ROOTFS_DIR}/var/lib/apt/lists/"*

if [[ -f "${APT_LIST}.keel-bak" ]]; then
  mv "${APT_LIST}.keel-bak" "${APT_LIST}"
fi

# The kiosk is not root. Chromium refuses to sandbox itself as root, and the
# alternative (--no-sandbox) would put an unsandboxed browser on the appliance.
#
# logind already grants seat devices by ACL, so these groups are a fallback for
# hardware where that does not happen. Filter to the ones this rootfs actually
# has: a missing group would otherwise fail the whole build over a fallback.
KIOSK_GROUPS=""
for group in video input render; do
  if chroot "${ROOTFS_DIR}" getent group "${group}" >/dev/null 2>&1; then
    KIOSK_GROUPS="${KIOSK_GROUPS:+${KIOSK_GROUPS},}${group}"
  fi
done

if ! chroot "${ROOTFS_DIR}" id keel-kiosk >/dev/null 2>&1; then
  chroot "${ROOTFS_DIR}" useradd --system --create-home \
    --home-dir /var/lib/keel-kiosk --shell /usr/sbin/nologin \
    ${KIOSK_GROUPS:+--groups "${KIOSK_GROUPS}"} --user-group keel-kiosk
fi

# systemd's PAMName needs a stack that opens a session; it does not authenticate.
# Upstream cage uses a dedicated minimal file rather than /etc/pam.d/login,
# whose Debian stack carries securetty, nologin, motd and lastlog behaviour that
# an appliance kiosk has no use for.
install -D -m 0644 /dev/stdin "${ROOTFS_DIR}/etc/pam.d/keel-kiosk" <<'EOF'
auth       required   pam_unix.so nullok
account    required   pam_unix.so
session    required   pam_unix.so
session    required   pam_systemd.so
EOF

# The login prompt in the photo comes from here. An appliance has no login
# prompt: masking the unit means tty1 belongs to the kiosk alone.
ln -sfn /dev/null "${ROOTFS_DIR}/etc/systemd/system/getty@tty1.service"

# Recovery still exists, it is just not advertised. Ctrl+Alt+F3 reaches a real
# console when the kiosk cannot start.
mkdir -p "${ROOTFS_DIR}/etc/systemd/system/getty.target.wants"
if [[ -e "${ROOTFS_DIR}/lib/systemd/system/getty@.service" ]]; then
  ln -sfn /lib/systemd/system/getty@.service \
    "${ROOTFS_DIR}/etc/systemd/system/getty.target.wants/getty@tty3.service"
elif [[ -e "${ROOTFS_DIR}/usr/lib/systemd/system/getty@.service" ]]; then
  ln -sfn /usr/lib/systemd/system/getty@.service \
    "${ROOTFS_DIR}/etc/systemd/system/getty.target.wants/getty@tty3.service"
fi
