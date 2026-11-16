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
# Where a visitor stops on the way to the front door page. This goes last,
# because the page keeps the end of what is written here.
show "Front door, path"
code() { curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "$1" 2>/dev/null || true; }
house=$(ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1)
inside=$(podman inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' keel-door 2>/dev/null | awk '{print $1}')
printf 'podman %s, network %s\n' "$(podman --version 2>/dev/null | awk '{print $3}')" "$(podman info --format '{{.Host.NetworkBackend}}' 2>/dev/null)"
podman ps -a --format '{{.Names}}: {{.Status}}: {{.Ports}}' 2>&1 | head -n 12
printf 'page from inside the program: %s\n' "$(podman exec keel-door curl -sS -o /dev/null --max-time 3 -w '%{http_code}' http://127.0.0.1:81/ 2>&1 | tail -n 1 | cut -c 1-120)"
printf 'page at the program address %s: %s\n' "${inside:-none}" "$(code "http://${inside:-0.0.0.0}:81/")"
printf 'page on this computer: %s\n' "$(code "http://127.0.0.1:81/")"
printf 'page at the house address %s: %s\n' "${house:-none}" "$(code "http://${house:-0.0.0.0}:81/")"
printf 'lookups inside the program: %s\n' "$(podman exec keel-door cat /etc/resolv.conf 2>&1 | grep -v '^#' | tr '\n' ' ' | cut -c 1-160)"
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
