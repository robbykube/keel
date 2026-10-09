#!/bin/sh
set -eu

# Debian Live normally configures Ethernet during boot. Starting dhcpcd here
# as well makes the Keel appliance contract explicit and covers systems whose
# live boot path did not retain the initramfs lease.
if ip -o -4 address show scope global | grep -q .; then
  exit 0
fi

/usr/sbin/dhcpcd -4 -b -q || true

attempt=0
while [ "${attempt}" -lt 30 ]; do
  if ip -o -4 address show scope global | grep -q .; then
    exit 0
  fi
  attempt=$((attempt + 1))
  sleep 1
done

# Keep booting so the local screen can explain that no address was obtained.
exit 0
