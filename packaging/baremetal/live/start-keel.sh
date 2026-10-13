#!/bin/sh
set -eu

TOKEN_FILE=/run/keel-admin-token
install -d -m 0700 /run/keel
install -d -m 0700 /var/lib/keel

if [ ! -s "${TOKEN_FILE}" ]; then
  TOKEN="keel-$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')"
  printf '%s\n' "${TOKEN}" > "${TOKEN_FILE}"
  chmod 0600 "${TOKEN_FILE}"
fi

install -d -m 0777 /var/lib/keel/files

if command -v id >/dev/null 2>&1 && ! id keel >/dev/null 2>&1; then
  useradd --system --home /var/lib/keel --shell /usr/sbin/nologin --user-group keel || true
fi
if command -v smbpasswd >/dev/null 2>&1 && id keel >/dev/null 2>&1; then
  printf '%s\n%s\n' keel keel | smbpasswd -a keel -s >/dev/null 2>&1 || true
fi

export KEEL_ADMIN_TOKEN="$(cat "${TOKEN_FILE}")"
export KEEL_BIND=0.0.0.0
export KEEL_PORT=9478
export KEEL_INSTALLER_ENABLED="${KEEL_INSTALLER_ENABLED:-1}"
exec /usr/bin/keel
