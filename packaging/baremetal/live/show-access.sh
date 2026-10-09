#!/bin/sh
set -eu

TOKEN_FILE=/run/keel-admin-token
TOKEN=""
ADDRESS=""

attempt=0
while [ "${attempt}" -lt 90 ]; do
  if [ -s "${TOKEN_FILE}" ]; then
    TOKEN="$(cat "${TOKEN_FILE}")"
  fi
  ADDRESS="$(ip -o -4 address show scope global 2>/dev/null | awk '{split($4, a, "/"); print a[1]; exit}')"
  # Do not hold the screen blank for the full 90s waiting on a token that keel
  # may never write. Once there is an address, give the token a short grace and
  # then print regardless: a banner saying the token is unavailable is far more
  # use than an empty screen.
  if [ -n "${ADDRESS}" ] && { [ -n "${TOKEN}" ] || [ "${attempt}" -ge 10 ]; }; then
    break
  fi
  attempt=$((attempt + 1))
  sleep 1
done

if [ -z "${ADDRESS}" ]; then
  ADDRESS="network address not available yet"
fi

{
  printf '\033[2J\033[H'
  printf '\n'
  printf '============================================================\n'
  printf '                         KEEL LIVE\n'
  printf '============================================================\n'
  printf '\n'
  printf '  Keel is running in temporary USB test mode.\n'
  printf '  No internal disk has been changed.\n'
  printf '\n'
  if [ "${ADDRESS}" = "network address not available yet" ]; then
    printf '  Network: %s\n' "${ADDRESS}"
  else
    printf '  On your Windows computer, open:\n'
    printf '  http://%s:9478/\n' "${ADDRESS}"
  fi
  printf '\n'
  printf '  Admin token: %s\n' "${TOKEN:-not available}"
  printf '\n'
  if [ "${ADDRESS}" != "network address not available yet" ]; then
    printf '  Files from Windows File Explorer:\n'
    printf '  \\\\%s\\files\n' "${ADDRESS}"
    printf '  Name: keel\n'
    printf '  Password: keel\n'
    printf '  (this house network only)\n'
    printf '\n'
  fi
  printf '  Shut down or restart to leave Keel Live.\n'
  printf '============================================================\n'
  printf '\n'
} > /dev/tty1
