#!/usr/bin/env bash
set -Eeuo pipefail

# Install key-only SSH into an unsquashed Debian live rootfs, for development.
#
# This exists so a running Keel VM or test machine can be worked on from the
# developer's desk: reading journals, and pushing a rebuilt binary in seconds
# with `keelctl bootstrap` instead of rebuilding a whole ISO.
#
# Passwords are refused outright. Debian live's built-in account has a password
# anyone can look up, so password SSH on this image would hand the machine to
# whoever is on the network. A public key is required; there is no fallback.
#
# No physical disk is written.

ROOTFS_DIR="${1:-}"
PUBKEY_INPUT="${2:-}"

if [[ -z "${ROOTFS_DIR}" || ! -d "${ROOTFS_DIR}" ]]; then
  echo "Usage: $0 /path/to/rootfs <public-key-file-or-literal>" >&2
  exit 2
fi

if [[ -z "${PUBKEY_INPUT}" ]]; then
  echo "A public key is required. Refusing to install SSH without one." >&2
  exit 2
fi

if [[ -f "${PUBKEY_INPUT}" ]]; then
  PUBKEY="$(cat "${PUBKEY_INPUT}")"
else
  PUBKEY="${PUBKEY_INPUT}"
fi

# Catch a private key, or a path typo that silently produced junk, before it is
# baked into an image.
if [[ ! "${PUBKEY}" =~ ^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp[0-9]+|sk-ssh-ed25519@openssh\.com|sk-ecdsa-sha2-nistp[0-9]+@openssh\.com)[[:space:]] ]]; then
  echo "That does not look like an SSH public key: ${PUBKEY:0:40}..." >&2
  echo "Expected a line starting with ssh-ed25519, ssh-rsa or similar." >&2
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
chroot "${ROOTFS_DIR}" apt-get install -y --no-install-recommends openssh-server
chroot "${ROOTFS_DIR}" apt-get clean
rm -rf "${ROOTFS_DIR}/var/lib/apt/lists/"*

if [[ -f "${APT_LIST}.keel-bak" ]]; then
  mv "${APT_LIST}.keel-bak" "${APT_LIST}"
fi

# Debian's sshd_config includes this directory, so a drop-in beats editing the
# shipped file.
install -D -m 0644 /dev/stdin "${ROOTFS_DIR}/etc/ssh/sshd_config.d/keel.conf" <<'EOF'
# Keel development image. Keys only.
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitEmptyPasswords no
PubkeyAuthentication yes
PermitRootLogin prohibit-password
X11Forwarding no
EOF

# keelctl runs everything through sudo, which succeeds as root, so root with a
# key is the account it can bootstrap with.
install -d -m 0700 "${ROOTFS_DIR}/root/.ssh"
printf '%s\n' "${PUBKEY}" > "${ROOTFS_DIR}/root/.ssh/authorized_keys"
chmod 0600 "${ROOTFS_DIR}/root/.ssh/authorized_keys"

# Host keys baked into the image would be identical on every machine that boots
# it, so every Keel USB would share one identity. Ship none and regenerate at
# first boot instead.
rm -f "${ROOTFS_DIR}"/etc/ssh/ssh_host_*
install -D -m 0644 /dev/stdin \
  "${ROOTFS_DIR}/etc/systemd/system/ssh.service.d/keel-host-keys.conf" <<'EOF'
[Service]
ExecStartPre=-/usr/sbin/ssh-keygen -A
EOF

mkdir -p "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants"
for unit in /lib/systemd/system/ssh.service /usr/lib/systemd/system/ssh.service; do
  if [[ -e "${ROOTFS_DIR}${unit}" ]]; then
    ln -sfn "${unit}" \
      "${ROOTFS_DIR}/etc/systemd/system/multi-user.target.wants/ssh.service"
    break
  fi
done

echo "SSH installed: key-only, root login by key, host keys generated at boot."
