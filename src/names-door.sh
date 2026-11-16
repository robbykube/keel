set -uo pipefail
cfg=/var/lib/npm
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
keep_log() {
  install -d -m 0755 /var/lib/keel
  {
    printf '%s\n' "---- $(date -Is) ----"
    printf 'localhost %s house %s\n' \
      "$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://127.0.0.1:81/" || true)" \
      "$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://${house:-}:81/" || true)"
    podman logs --tail 120 keel-door 2>&1 || true
  } >> /var/lib/keel/door.log 2>/dev/null || true
}
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/door.json.tmp
  mv /run/keel/door.json.tmp /run/keel/door.json
}
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
page_ok() {
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "$1" || true)
  case "$code" in
    2*|3*) return 0 ;;
  esac
  return 1
}
ready() {
  page_ok "http://127.0.0.1:81/" && page_ok "http://$house:81/"
}
finish() {
  cat > /etc/systemd/system/keel-door.service <<'EOF'
[Unit]
Description=Keel front door
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStartPre=-/bin/sh -c 'sysctl -w net.ipv4.ip_forward=1; sysctl -w net.ipv4.conf.all.route_localnet=1; iptables -P FORWARD ACCEPT; iptables -C FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT || iptables -I FORWARD 1 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT; iptables -C FORWARD -p tcp -m multiport --dports 80,81,443 -j ACCEPT || iptables -I FORWARD 1 -p tcp -m multiport --dports 80,81,443 -j ACCEPT'
ExecStart=/usr/bin/podman start keel-door
ExecStop=/usr/bin/podman stop -t 10 keel-door
[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload >"$log" 2>&1 || true
  systemctl enable keel-door.service >"$log" 2>&1 || true
  if podman container exists keel-photos-server >/dev/null 2>&1; then
    podman network connect keel-photos keel-door >"$log" 2>&1 || true
  fi
  if podman container exists keel-jitsi-web >/dev/null 2>&1; then
    podman network connect keel-jitsi keel-door >"$log" 2>&1 || true
  fi
  if podman container exists keel-passwords >/dev/null 2>&1; then
    podman network connect keel-passwords keel-door >"$log" 2>&1 || true
  fi
  if podman container exists keel-papers >/dev/null 2>&1; then
    podman network connect keel-papers keel-door >"$log" 2>&1 || true
  fi
  if podman container exists keel-mail >/dev/null 2>&1; then
    podman network connect keel-mail keel-door >"$log" 2>&1 || true
  fi
  touch "$cfg/installed"
  printf '%s\n' "$house"
  exit 0
}
mark 8 "Opening the front door."
install -d -m 0755 "$cfg/data" "$cfg/letsencrypt" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 18 "Installing the program that answers a name."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install the front door. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so the front door did not start."
image=docker.io/jc21/nginx-proxy-manager:2.16.0
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 45 "Downloading the front door. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download the front door. Check the network and try again."
else
  mark 45 "Opening the front door."
fi
mark 75 "Starting the front door."
# The page is inside the program. These let this computer pass a visitor through to it.
install -d -m 0755 /etc/sysctl.d
printf '%s\n' 'net.ipv4.ip_forward=1' 'net.ipv4.conf.all.route_localnet=1' > /etc/sysctl.d/keel-forward.conf
sysctl -w net.ipv4.ip_forward=1 >/dev/null 2>&1 || true
sysctl -w net.ipv4.conf.all.route_localnet=1 >/dev/null 2>&1 || true
iptables -P FORWARD ACCEPT >/dev/null 2>&1 || true
iptables -C FORWARD -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT >/dev/null 2>&1 || iptables -I FORWARD 1 -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT >/dev/null 2>&1 || true
iptables -C FORWARD -p tcp -m multiport --dports 80,81,443 -j ACCEPT >/dev/null 2>&1 || iptables -I FORWARD 1 -p tcp -m multiport --dports 80,81,443 -j ACCEPT >/dev/null 2>&1 || true
if podman container exists keel-door >/dev/null 2>&1; then
  podman start keel-door >"$log" 2>&1 || true
  i=0
  while [ "$i" -lt 15 ]; do
    ready && finish
    i=$((i + 1))
    sleep 2
  done
fi
# A start that never answered still left its files. The next Open would
# keep them, and the page would fail the same way.
rm -f "$cfg/installed"
keep_log
podman rm -f keel-door >"$log" 2>&1 || true
rm -rf "$cfg/data" "$cfg/letsencrypt"
install -d -m 0755 "$cfg/data" "$cfg/letsencrypt"
if ! podman network exists keel-front >/dev/null 2>&1; then
  podman network create --ipv6=false keel-front >"$log" 2>&1 || podman network create keel-front >"$log" 2>&1 || fail "Keel could not start the front door. Try Open again."
fi
if ! podman run -d --name keel-door --hostname keel-door --network keel-front \
  -p 80:80 -p 81:81 -p "$house:443:443" \
  -e DISABLE_IPV6=true \
  -e TZ=UTC \
  -v "$cfg/data:/data" \
  -v "$cfg/letsencrypt:/etc/letsencrypt" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering on port 80 or 443. The front door did not start."
  fi
  last=$(sed '/^$/d' "$log" | tail -n 1 | cut -c 1-140)
  fail "${last:-Keel could not start the front door. Try Open again.}"
fi
mark 90 "Waiting for the front door."
i=0
while [ "$i" -lt 60 ]; do
  ready && finish
  i=$((i + 1))
  sleep 2
done
if ! podman inspect -f '{{.State.Running}}' keel-door 2>/dev/null | grep -qx true; then
  last=$(podman logs --tail 20 keel-door 2>&1 | sed '/^$/d' | tail -n 1 | cut -c 1-160)
  fail "${last:-The front door stopped before the page was ready. Try Open again.}"
fi
keep_log
last=$(podman logs --tail 80 keel-door 2>&1 | sed '/^$/d' | grep -Ei 'emerg|address family|bind\(\)|failed' | tail -n 1 | cut -c 1-140)
fail "The front door started, but the page did not answer. ${last:-Try Open again.}"
