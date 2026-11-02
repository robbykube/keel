set -uo pipefail
cfg=/var/lib/technitium
log="$cfg/setup.log"
fail() { echo "$1" >&2; exit 1; }
mark() {
  install -d -m 0755 /run/keel
  printf '{"state":"running","percent":%s,"message":"%s","address":"","ready":false}\n' "$1" "$2" > /run/keel/names.json.tmp
  mv /run/keel/names.json.tmp /run/keel/names.json
}
house_of() {
  ip -4 route get 1.1.1.1 2>/dev/null | sed -n 's/.* src \([0-9.]*\).*/\1/p' | head -n 1
}
mark 8 "Opening Names."
install -d -m 0755 "$cfg/config" /run/keel
if ! command -v podman >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
  mark 18 "Installing the program that answers names."
  apt-get update -o Acquire::ForceIPv4=true >"$log" 2>&1 || fail "Keel could not download Names. Check the network and try again."
  DEBIAN_FRONTEND=noninteractive apt-get install -y -o Acquire::ForceIPv4=true podman curl >"$log" 2>&1 || fail "Keel could not install Names. Try Open again."
fi
house=$(house_of)
printf '%s\n' "$house" | awk -F. 'NF==4 && $1!~/^(0|127)$/ && !($1==169 && $2==254) && !($1==100 && $2>=64 && $2<=127) { ok=1 } END { exit ok?0:1 }' || fail "Keel could not see the house network, so Names did not start."
image=docker.io/technitium/dns-server:15.6.2
if ! podman image exists "$image" >/dev/null 2>&1; then
  mark 45 "Downloading Names. This can take a while."
  podman pull "$image" >"$log" 2>&1 || fail "Keel could not download Names. Check the network and try again."
else
  mark 45 "Opening Names."
fi
mark 75 "Starting Names."
podman rm -f keel-names >"$log" 2>&1 || true
if ! podman run -d --name keel-names --hostname keel-names \
  -p "$house:53:53/tcp" -p "$house:53:53/udp" -p "$house:5380:5380" \
  -e DNS_SERVER_DOMAIN=keel \
  -e TZ=UTC \
  -v "$cfg/config:/etc/dns" \
  "$image" >"$log" 2>&1; then
  if grep -qi "address already in use" "$log"; then
    fail "This computer is already answering names. Keel left that in place."
  fi
  fail "Keel could not start Names. Try Open again."
fi
mark 90 "Waiting for the names page."
up=0
i=0
while [ "$i" -lt 40 ]; do
  code=$(curl -sS -o /dev/null --max-time 3 -w '%{http_code}' "http://$house:5380/" || true)
  case "$code" in
    2*|3*) up=1; break ;;
  esac
  i=$((i + 1))
  sleep 2
done
[ "$up" = "1" ] || fail "Names started, but the page did not answer. Try Open again."
cat > /etc/systemd/system/keel-names.service <<'EOF'
[Unit]
Description=Keel names
After=network-online.target
Wants=network-online.target
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/podman start keel-names
ExecStop=/usr/bin/podman stop -t 10 keel-names
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload >"$log" 2>&1 || true
systemctl enable keel-names.service >"$log" 2>&1 || true
touch "$cfg/installed"
printf '%s\n' "$house"
exit 0
