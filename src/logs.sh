set -uo pipefail
show() {
  printf '\n%s\n' "$1"
  printf '%s\n' "--------------------------------"
}
show "Front door"
journalctl -u keel-door -n 80 --no-pager 2>&1 | tail -n 80 || true
if [ -f /var/lib/keel/door.log ]; then
  show "Front door, kept"
  tail -n 80 /var/lib/keel/door.log || true
fi
if [ -f /var/lib/keel/door-setup.log ]; then
  show "Front door setup"
  tail -n 40 /var/lib/keel/door-setup.log || true
fi
show "Photos copies"
# Each copy Photos made, and how long it took. Reading is most of it.
journalctl -u keel --no-pager -n 4000 2>/dev/null | grep "picture copy" | tail -n 30 || true
show "Names"
podman logs --tail 80 keel-names 2>&1 | tail -n 80 || true
show "Photos"
if [ -f /var/lib/immich/setup.log ]; then
  tail -n 20 /var/lib/immich/setup.log || true
fi
df -Pk /run / 2>/dev/null | awk 'NR==1 || $6=="/run" || $6=="/" { print }'
# What takes the temporary space, largest last.
du -xsk /run/* 2>/dev/null | sort -n | tail -n 5 || true
# Where a visitor stops on the way to the front door page. This goes last,
# because the page keeps the end of what is written here.
show "Front door, path"
code() { curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "$1" 2>/dev/null || true; }
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
printf 'caddy %s\n' "$(/usr/local/bin/caddy version 2>/dev/null | head -n 1 | cut -c 1-80)"
podman ps -a --format '{{.Names}}: {{.Status}}: {{.Ports}}' 2>&1 | head -n 12
printf 'page on this computer: %s\n' "$(code "http://127.0.0.1/")"
if [ -n "${house:-}" ]; then
  printf 'page at the house address %s: %s\n' "$house" "$(code "http://$house/")"
fi
printf 'lookups on this computer: %s\n' "$(grep -v '^#' /etc/resolv.conf 2>&1 | tr '\n' ' ' | cut -c 1-160)"
printf 'forward %s, localnet %s\n' "$(sysctl -n net.ipv4.ip_forward 2>/dev/null)" "$(sysctl -n net.ipv4.conf.all.route_localnet 2>/dev/null)"
show "Listening"
ss -H -ltnup 2>/dev/null | awk '$5 ~ /:(53|80|81|443|3000|5380)$/ { print $1, $5, $NF }' | cut -c 1-140 | head -n 30
show "Rules"
if command -v nft >/dev/null 2>&1; then
  nft list ruleset 2>&1 | cut -c 1-200 | head -n 140
else
  iptables -S 2>&1 | head -n 60
  iptables -t nat -S 2>&1 | head -n 60
fi
exit 0
