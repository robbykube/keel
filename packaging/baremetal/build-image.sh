#!/usr/bin/env bash
set -Eeuo pipefail

# Build-host-only scaffold for the Keel bare-metal pipeline.
# Run this on Debian/Ubuntu Linux, not on the target machine.
# It intentionally never partitions or writes a target disk.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT_DIR}/dist/baremetal"
# live-build rejects build paths containing spaces. Keep its working tree in
# WSL's temporary filesystem; only the final release artifacts go to the repo.
BUILD_DIR="${KEEL_LIVE_BUILD_DIR:-/tmp/keel-live-build}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Keel image builds must run on Linux (native, WSL2, or a Linux VM)." >&2
  exit 2
fi

command -v lb >/dev/null || {
  echo "Missing live-build. Install it on the build host, then rerun." >&2
  exit 2
}

command -v cargo >/dev/null || {
  echo "Missing cargo. Install Rust on the build host, then rerun." >&2
  exit 2
}

echo "Building the Keel release binary..."
cargo build --release --manifest-path "${ROOT_DIR}/Cargo.toml"

KEEL_BINARY="${ROOT_DIR}/target/release/keel"
if [[ ! -x "${KEEL_BINARY}" ]]; then
  echo "Expected release binary was not created: ${KEEL_BINARY}" >&2
  exit 1
fi

case "${BUILD_DIR}" in
  /tmp/keel-live-build|/tmp/keel-live-build-*)
    rm -rf -- "${BUILD_DIR}"
    ;;
  *)
    echo "Refusing to clean unexpected build directory: ${BUILD_DIR}" >&2
    exit 2
    ;;
esac

mkdir -p "${BUILD_DIR}/config/package-lists"
mkdir -p "${BUILD_DIR}/config/includes.chroot/usr/bin"
mkdir -p "${BUILD_DIR}/config/includes.chroot/usr/lib/keel"
mkdir -p "${BUILD_DIR}/config/includes.chroot/etc/systemd/system"

cp "${ROOT_DIR}/packaging/baremetal/keel-packages.list" \
  "${BUILD_DIR}/config/package-lists/keel.list.chroot"
cp "${KEEL_BINARY}" "${BUILD_DIR}/config/includes.chroot/usr/bin/keel"
cp "${ROOT_DIR}/packaging/systemd/keel.service" \
  "${BUILD_DIR}/config/includes.chroot/etc/systemd/system/keel.service"
cp "${ROOT_DIR}/packaging/baremetal/install-keel" \
  "${BUILD_DIR}/config/includes.chroot/usr/lib/keel/install-keel"
chmod 0755 "${BUILD_DIR}/config/includes.chroot/usr/lib/keel/install-keel"
mkdir -p "${BUILD_DIR}/config/includes.chroot/usr/lib/keel/web"
cp -a "${ROOT_DIR}/web/." "${BUILD_DIR}/config/includes.chroot/usr/lib/keel/web/"

# Keep a copy of the source contract in the image for recovery and support.
cp "${ROOT_DIR}/packaging/baremetal/README.md" \
  "${BUILD_DIR}/config/includes.chroot/usr/lib/keel/INSTALLATION.md"

cd "${BUILD_DIR}"

# live-build creates the bootable installer/live environment. The actual
# disk-install transaction will be added as a separate installer component;
# keeping it separate prevents an image build from ever touching host disks.
lb config \
  --distribution trixie \
  --architectures amd64 \
  --archive-areas "main contrib non-free non-free-firmware" \
  --mirror-bootstrap "http://deb.debian.org/debian" \
  --mirror-chroot "http://deb.debian.org/debian" \
  --mirror-binary "http://deb.debian.org/debian" \
  --mirror-chroot-security "http://security.debian.org/debian-security" \
  --mirror-binary-security "http://security.debian.org/debian-security" \
  --linux-packages "linux-image" \
  --linux-flavours "amd64" \
  --binary-images iso-hybrid \
  --debian-installer live \
  --bootappend-live "boot=live components" \
  --memtest none

if [[ "${1:-}" == "--prepare" ]]; then
  echo "Build configuration prepared in ${BUILD_DIR}."
  echo "No target disk was modified."
  exit 0
fi

echo "Building the bootable Keel ISO..."
lb build

mkdir -p "${OUT_DIR}/release"
mv "live-image-amd64.hybrid.iso" "${OUT_DIR}/release/keel-amd64.iso"
sha256sum "${OUT_DIR}/release/keel-amd64.iso" \
  > "${OUT_DIR}/release/keel-amd64.iso.sha256"

echo "Created ${OUT_DIR}/release/keel-amd64.iso"
echo "Checksum: ${OUT_DIR}/release/keel-amd64.iso.sha256"
echo "No target disk was modified."
