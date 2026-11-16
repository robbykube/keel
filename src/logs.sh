set -uo pipefail
show() {
  printf '\n%s\n' "$1"
  printf '%s\n' "--------------------------------"
}
show "Front door"
podman logs --tail 100 keel-door 2>&1 | tail -n 100 || true
if [ -f /var/lib/keel/door.log ]; then
  show "Front door, kept"
  tail -n 160 /var/lib/keel/door.log || true
fi
if [ -f /var/lib/npm/setup.log ]; then
  show "Front door setup"
  tail -n 40 /var/lib/npm/setup.log || true
fi
show "Names"
podman logs --tail 80 keel-names 2>&1 | tail -n 80 || true
exit 0
