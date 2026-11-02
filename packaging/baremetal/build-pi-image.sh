#!/bin/bash
# Build a Raspberry Pi card image. Writing this file to the microSD is the install.
# Pi 4 and Pi 5, 64-bit. The family folders stay on the USB disks.
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="${KEEL_PI_WORK:-/var/tmp/keel-pi-build}"
ROOT="${WORK}/rootfs"
MNT="${WORK}/mnt"
IMG_TMP="${WORK}/keel-arm64-pi.img"
OUT_DIR="${ROOT_DIR}/dist/baremetal/release"
SUITE="${KEEL_DEBIAN_SUITE:-trixie}"
MIRROR="${KEEL_DEBIAN_MIRROR:-http://deb.debian.org/debian}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Build this from Debian or WSL: packaging/baremetal/build-pi-image.ps1" >&2
  exit 1
fi
if [[ "${EUID}" -ne 0 ]]; then
  echo "Run as root." >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
  debootstrap qemu-user-static gcc-aarch64-linux-gnu libc6-dev-arm64-cross \
  parted dosfstools e2fsprogs rsync curl ca-certificates \
  binfmt-support device-tree-compiler

if ! rustup target list --installed | grep -q '^aarch64-unknown-linux-gnu$'; then
  rustup target add aarch64-unknown-linux-gnu
fi

echo "Building the arm64 Keel program..."
export CARGO_TARGET_DIR="${KEEL_PI_CARGO_TARGET:-/var/tmp/keel-pi-target}"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
cargo build --release --target aarch64-unknown-linux-gnu --manifest-path "${ROOT_DIR}/Cargo.toml"
KEEL_BINARY="${CARGO_TARGET_DIR}/aarch64-unknown-linux-gnu/release/keel"
[[ -x "${KEEL_BINARY}" ]] || { echo "The arm64 program was not produced." >&2; exit 1; }

if [[ "$(uname -m)" != "aarch64" && ! -f /proc/sys/fs/binfmt_misc/qemu-aarch64 ]]; then
  echo "qemu-user-binfmt is installed, but arm64 programs cannot run. Enable binfmt and run the build again." >&2
  exit 1
fi

chroot_run() {
  chroot "${ROOT}" "$@"
}

if [[ "${KEEL_PI_REUSE:-0}" != "1" || ! -x "${ROOT}/bin/bash" ]]; then
  echo "Creating the Debian system..."
  rm -rf "${ROOT}"
  mkdir -p "${ROOT}"
  debootstrap --arch=arm64 --foreign --variant=minbase "${SUITE}" "${ROOT}" "${MIRROR}"
  chroot_run /debootstrap/debootstrap --second-stage

  cat > "${ROOT}/etc/apt/sources.list" <<EOF
deb ${MIRROR} ${SUITE} main contrib non-free-firmware
deb http://security.debian.org/debian-security ${SUITE}-security main contrib non-free-firmware
EOF
  if [[ -r /etc/resolv.conf ]]; then
    cp /etc/resolv.conf "${ROOT}/etc/resolv.conf"
  fi
  mkdir -p "${ROOT}/proc" "${ROOT}/sys" "${ROOT}/dev/pts" "${ROOT}/boot/firmware"
  mount -t proc proc "${ROOT}/proc"
  mount -t sysfs sysfs "${ROOT}/sys"
  mount --bind /dev "${ROOT}/dev"
  mount --bind /dev/pts "${ROOT}/dev/pts" || true

  chroot_run apt-get update
  chroot_run apt-get install -y --no-install-recommends \
    systemd systemd-sysv dbus libpam-systemd udev \
    ca-certificates curl iproute2 util-linux passwd \
    openssh-server \
    linux-image-arm64 raspi-firmware initramfs-tools \
    systemd-resolved systemd-timesyncd \
    samba \
    smartmontools ethtool parted e2fsprogs cloud-guest-utils \
    cage sway chromium libgl1-mesa-dri fonts-dejavu-core
  chroot_run apt-get clean
  rm -rf "${ROOT}/var/lib/apt/lists/"*
else
  echo "Reusing the prepared system in ${ROOT}"
  mkdir -p "${ROOT}/proc" "${ROOT}/sys" "${ROOT}/dev/pts" "${ROOT}/boot/firmware"
  mountpoint -q "${ROOT}/proc" || mount -t proc proc "${ROOT}/proc"
  mountpoint -q "${ROOT}/sys" || mount -t sysfs sysfs "${ROOT}/sys"
  mountpoint -q "${ROOT}/dev" || mount --bind /dev "${ROOT}/dev"
  mountpoint -q "${ROOT}/dev/pts" || mount --bind /dev/pts "${ROOT}/dev/pts" || true
fi

if ! chroot_run dpkg -s sway >/dev/null 2>&1; then
  echo "Installing the screen program..."
  rm -f "${ROOT}/etc/resolv.conf"
  cp /etc/resolv.conf "${ROOT}/etc/resolv.conf"
  chroot_run apt-get update
  chroot_run apt-get install -y --no-install-recommends sway
  chroot_run apt-get clean
  rm -rf "${ROOT}/var/lib/apt/lists/"*
fi

echo "Installing Keel..."
install -D -m 0755 "${KEEL_BINARY}" "${ROOT}/usr/bin/keel"
install -D -m 0644 "${ROOT_DIR}/packaging/systemd/keel.service" \
  "${ROOT}/etc/systemd/system/keel.service"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/rollback-update" \
  "${ROOT}/usr/lib/keel/rollback-update"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/pi/first-boot" \
  "${ROOT}/usr/lib/keel/first-boot"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/pi/grow-root" \
  "${ROOT}/usr/lib/keel/grow-root"
install -D -m 0755 "${ROOT_DIR}/packaging/baremetal/pi/kiosk.sh" \
  "${ROOT}/usr/lib/keel/start-kiosk.sh"
install -D -m 0644 "${ROOT_DIR}/packaging/systemd/keel-apply-update.service" \
  "${ROOT}/etc/systemd/system/keel-apply-update.service"
install -D -m 0644 "${ROOT_DIR}/packaging/systemd/keel-apply-update.path" \
  "${ROOT}/etc/systemd/system/keel-apply-update.path"
install -D -m 0644 "${ROOT_DIR}/packaging/systemd/keel-update-rollback.service" \
  "${ROOT}/etc/systemd/system/keel-update-rollback.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/pi/keel-first-boot.service" \
  "${ROOT}/etc/systemd/system/keel-first-boot.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/pi/keel-grow-root.service" \
  "${ROOT}/etc/systemd/system/keel-grow-root.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/keel-kiosk.service" \
  "${ROOT}/etc/systemd/system/keel-kiosk.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/pi/keel-screen-plug.service" \
  "${ROOT}/etc/systemd/system/keel-screen-plug.service"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/pi/80-keel-screen.rules" \
  "${ROOT}/etc/udev/rules.d/80-keel-screen.rules"
install -D -m 0644 "${ROOT_DIR}/packaging/baremetal/live/smb.conf" \
  "${ROOT}/etc/samba/smb.conf"
mkdir -p "${ROOT}/etc/systemd/system/keel.service.d" "${ROOT}/usr/lib/keel/web" "${ROOT}/var/lib/keel" "${ROOT}/etc/keel"
cat > "${ROOT}/etc/systemd/system/keel.service.d/installed.conf" <<'EOF'
[Unit]
After=keel-first-boot.service
Wants=keel-first-boot.service

[Service]
Environment=KEEL_TRUST_LOOPBACK=1
EOF
rsync -a --delete "${ROOT_DIR}/web/" "${ROOT}/usr/lib/keel/web/"
find "${ROOT}/usr/lib/keel" -type f \( -name '*.sh' -o -name 'first-boot' -o -name 'grow-root' -o -name 'rollback-update' \) \
  -exec sed -i 's/\r$//' {} +
sed -i 's/\r$//' \
  "${ROOT}/etc/systemd/system/keel-screen-plug.service" \
  "${ROOT}/etc/udev/rules.d/80-keel-screen.rules"
chmod 0755 "${ROOT}/usr/lib/keel/first-boot" "${ROOT}/usr/lib/keel/grow-root" "${ROOT}/usr/lib/keel/rollback-update" "${ROOT}/usr/lib/keel/start-kiosk.sh"

printf '%s\n' '# Keel writes the folders here.' > "${ROOT}/var/lib/keel/smb-shares.conf"
echo keel > "${ROOT}/etc/hostname"
cat > "${ROOT}/etc/hosts" <<'EOF'
127.0.0.1 localhost
127.0.1.1 keel
::1 localhost ip6-localhost ip6-loopback
ff02::1 ip6-allnodes
ff02::2 ip6-allrouters
EOF
cat > "${ROOT}/etc/fstab" <<'EOF'
LABEL=keel-root / ext4 defaults,noatime 0 1
LABEL=KEEL-BOOT /boot/firmware vfat defaults 0 2
EOF

# The sign-in token is created on the first boot of each card, not baked in.
rm -f "${ROOT}/etc/keel/keel.env" "${ROOT}/var/lib/keel/admin-token"
: > "${ROOT}/etc/machine-id"
rm -f "${ROOT}/var/lib/dbus/machine-id" "${ROOT}/etc/ssh/ssh_host_"*

if ! chroot_run id keel-kiosk >/dev/null 2>&1; then
  groups=""
  for group in video input render; do
    if chroot_run getent group "${group}" >/dev/null 2>&1; then
      groups="${groups:+${groups},}${group}"
    fi
  done
  chroot_run useradd --system --create-home \
    --home-dir /var/lib/keel-kiosk --shell /usr/sbin/nologin \
    ${groups:+--groups "${groups}"} --user-group keel-kiosk
fi
install -D -m 0644 /dev/stdin "${ROOT}/etc/pam.d/keel-kiosk" <<'EOF'
auth       required   pam_unix.so nullok
account    required   pam_unix.so
session    required   pam_unix.so
session    required   pam_systemd.so
EOF
ln -sfn /dev/null "${ROOT}/etc/systemd/system/getty@tty1.service"
mkdir -p "${ROOT}/etc/systemd/system/getty.target.wants"
if [[ -e "${ROOT}/usr/lib/systemd/system/getty@.service" ]]; then
  ln -sfn /usr/lib/systemd/system/getty@.service \
    "${ROOT}/etc/systemd/system/getty.target.wants/getty@tty3.service"
fi

systemctl --root="${ROOT}" enable keel.service
systemctl --root="${ROOT}" enable keel-first-boot.service
systemctl --root="${ROOT}" enable keel-grow-root.service
systemctl --root="${ROOT}" enable keel-kiosk.service
systemctl --root="${ROOT}" enable keel-apply-update.path
systemctl --root="${ROOT}" enable keel-update-rollback.service
systemctl --root="${ROOT}" enable systemd-networkd.service
systemctl --root="${ROOT}" enable systemd-resolved.service
systemctl --root="${ROOT}" enable systemd-timesyncd.service
systemctl --root="${ROOT}" enable smbd.service

mkdir -p "${ROOT}/etc/systemd/network"
cat > "${ROOT}/etc/systemd/network/20-wired.network" <<'EOF'
[Match]
Type=ether

[Network]
DHCP=yes
EOF
ln -sfn /run/systemd/resolve/stub-resolv.conf "${ROOT}/etc/resolv.conf"

shopt -s nullglob
kernels=("${ROOT}/boot/firmware"/vmlinuz-*)
initrds=("${ROOT}/boot/firmware"/initrd.img-*)
shopt -u nullglob
if [[ ${#kernels[@]} -eq 0 || ${#initrds[@]} -eq 0 ]]; then
  echo "The Pi boot files were not written. Expected a kernel and initrd in /boot/firmware." >&2
  find "${ROOT}/boot/firmware" -maxdepth 2 -type f >&2 || true
  exit 1
fi
# The boot partition is FAT, which cannot store a symlink. config.txt names
# the versioned kernel and initrd directly.
if ! compgen -G "${ROOT}/boot/firmware/*.dtb" > /dev/null; then
  echo "No Pi device tree was copied into the boot files." >&2
  find "${ROOT}/boot/firmware" -maxdepth 2 -type f >&2 || true
  exit 1
fi
cat > "${ROOT}/boot/firmware/cmdline.txt" <<'EOF'
console=tty1 root=LABEL=keel-root rw rootwait fsck.repair=yes quiet loglevel=3 systemd.show_status=false cma=128M
EOF
if [[ ! -f "${ROOT}/boot/firmware/config.txt" ]] || ! grep -q 'kernel=vmlinuz' "${ROOT}/boot/firmware/config.txt"; then
  cat > "${ROOT}/boot/firmware/config.txt" <<'EOF'
arm_64bit=1
enable_uart=1
kernel=vmlinuz
initramfs initrd.img followkernel
EOF
fi
if ! grep -q 'initramfs' "${ROOT}/boot/firmware/config.txt"; then
  printf '%s\n' 'initramfs initrd.img followkernel' >> "${ROOT}/boot/firmware/config.txt"
fi
# HDMI is already in the Pi 4 tree. The official small panel is not, and
# Debian does not ship the overlay that would turn it on. This one uses the
# names in Debian's tree. The firmware rewrites config.txt when the kernel
# updates, then appends raspi-firmware-custom, so the line lives there too.
mkdir -p "${ROOT}/boot/firmware/overlays" "${ROOT}/usr/lib/raspi-firmware/overlays"
sed 's/\r$//' "${ROOT_DIR}/packaging/baremetal/pi/keel-dsi-7inch.dts" \
  | dtc -@ -I dts -O dtb -o "${ROOT}/boot/firmware/overlays/keel-dsi-7inch.dtbo"
cp -a "${ROOT}/boot/firmware/overlays/keel-dsi-7inch.dtbo" \
  "${ROOT}/usr/lib/raspi-firmware/overlays/keel-dsi-7inch.dtbo"
printf '%s\n' 'display_auto_detect=1' 'dtoverlay=keel-dsi-7inch' > "${ROOT}/etc/default/raspi-firmware-custom"
if ! grep -q '^display_auto_detect=' "${ROOT}/boot/firmware/config.txt"; then
  printf '\n%s\n' '# Inserted by /etc/default/raspi-firmware-custom' 'display_auto_detect=1' >> "${ROOT}/boot/firmware/config.txt"
fi
if ! grep -q '^dtoverlay=keel-dsi-7inch$' "${ROOT}/boot/firmware/config.txt"; then
  printf '%s\n' 'dtoverlay=keel-dsi-7inch' >> "${ROOT}/boot/firmware/config.txt"
fi

rm -f "${ROOT}/usr/bin/qemu-aarch64-static"
umount "${ROOT}/dev/pts" || true
umount "${ROOT}/dev" || true
umount "${ROOT}/proc" || true
umount "${ROOT}/sys" || true

echo "Writing the card image..."
root_mb="$(du -sm "${ROOT}" | awk '{print $1}')"
img_mb="$((root_mb + 1600))"
rm -f "${IMG_TMP}"
truncate -s "${img_mb}M" "${IMG_TMP}"
parted -s "${IMG_TMP}" mklabel msdos
parted -s "${IMG_TMP}" mkpart primary fat32 4MiB 516MiB
parted -s "${IMG_TMP}" mkpart primary ext4 516MiB 100%
parted -s "${IMG_TMP}" set 1 boot on
LOOP="$(losetup --find --show -P "${IMG_TMP}")"
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  if [[ -b "${LOOP}p1" && -b "${LOOP}p2" ]]; then
    break
  fi
  sleep 0.5
done
if [[ ! -b "${LOOP}p1" || ! -b "${LOOP}p2" ]]; then
  echo "The card image partitions did not appear." >&2
  exit 1
fi
trap 'umount -l "${MNT}/boot/firmware" 2>/dev/null || true; umount -l "${MNT}" 2>/dev/null || true; losetup -d "${LOOP}" 2>/dev/null || true' EXIT
mkfs.vfat -F 32 -n KEEL-BOOT "${LOOP}p1"
mkfs.ext4 -F -L keel-root "${LOOP}p2"
mkdir -p "${MNT}"
mount "${LOOP}p2" "${MNT}"
mkdir -p "${MNT}/boot/firmware"
mount "${LOOP}p1" "${MNT}/boot/firmware"
rsync -aH --exclude '/boot/firmware/***' "${ROOT}/" "${MNT}/"
rsync -a "${ROOT}/boot/firmware/" "${MNT}/boot/firmware/"
sync
umount "${MNT}/boot/firmware"
umount "${MNT}"
losetup -d "${LOOP}"
trap - EXIT

mkdir -p "${OUT_DIR}"
mv -f "${IMG_TMP}" "${OUT_DIR}/keel-arm64-pi.img"
sha256sum "${OUT_DIR}/keel-arm64-pi.img" > "${OUT_DIR}/keel-arm64-pi.img.sha256"
echo "Created ${OUT_DIR}/keel-arm64-pi.img"
cat "${OUT_DIR}/keel-arm64-pi.img.sha256"
